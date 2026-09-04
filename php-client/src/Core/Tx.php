<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Legacy transaction message compilation, and the wire framing around it.
 *
 * `Ix` stops at an instruction. Between an instruction and something a
 * validator will accept there is a message to compile -- compact-u16 length
 * prefixes, account-key deduplication and ordering, the three header counts,
 * the program-id index, the recent blockhash -- and then a signature array in
 * front of it. This is that step and nothing more.
 *
 * Two pure functions of their arguments, the property the rest of `src/Core`
 * is built on. Nothing here fetches a blockhash, signs, verifies a signature,
 * talks to an RPC endpoint, or retries: see wasm-client/SPEC.md §7. The
 * signature comes from the caller, which on a PHP server means
 * `sodium_crypto_sign_detached` -- ext-sodium's *signature* API, present
 * since 7.2. (The ed25519 core-point API it does not expose is a different
 * function and a different problem; see `Ed25519`.)
 *
 * **Legacy messages only.** Versioned (v0) messages and address lookup tables
 * are out of scope, not unimplemented: a lookup table trades bytes for a
 * round trip and a table to maintain, which a metering call carrying nine
 * accounts does not need. There is no version byte here and no way to ask for
 * one.
 *
 * Every byte this produces is checked against `solana-message` and
 * `solana-transaction` output on every conformance run, over three cases
 * chosen to reach each branch below -- see `php-client/README.md`, "The order
 * this has to happen in". That check exists because a divergent message
 * serializer does not fail cleanly: it builds a plausible transaction that
 * does the wrong thing, and then somebody signs it.
 */
final class Tx
{
    /** A compiled instruction indexes the key list with single bytes. */
    private const MAX_ACCOUNTS = 255;

    /**
     * Compile instructions into a legacy message, ready to be signed.
     *
     * The returned value is the exact byte string a signer signs over and the
     * one `wire()` expects back -- binary, not base58 or hex, same as
     * `Instruction::$data`.
     *
     * `$feePayer` and `$recentBlockhash` are base58, like every address that
     * crosses this package's boundary. The blockhash is supplied, never
     * fetched.
     *
     * @param Instruction[] $instructions in the order the program requires;
     *                                    compilation preserves it
     *
     * @throws TxException on an address that is not 32 bytes, or more than
     *                     255 distinct accounts
     */
    public static function compile(array $instructions, string $feePayer, string $recentBlockhash): string
    {
        // Every key's flags, OR'd across every instruction that names it. A
        // key writable in one instruction and readonly in another is writable
        // in the message: the flags describe what the transaction as a whole
        // may do, not what one instruction asked for.
        $meta = [];
        foreach ($instructions as $instruction) {
            foreach ($instruction->accounts as $account) {
                $was = $meta[$account->pubkey] ?? [false, false];
                $meta[$account->pubkey] = [
                    $was[0] || $account->isSigner,
                    $was[1] || $account->isWritable,
                ];
            }
        }

        // Called programs are accounts of the transaction too, and readonly
        // unless an instruction separately named one as writable. `??=` keeps
        // whatever the loop above already decided.
        foreach ($instructions as $instruction) {
            $meta[$instruction->programId] ??= [false, false];
        }

        // The fee payer signs and is written whatever the instructions said.
        // `meter_and_settle` marks the site authority a *readonly* signer and
        // the same key usually pays, so this promotion is the normal case
        // here rather than an edge one.
        $meta[$feePayer] = [true, true];

        // Four partitions, and inside each one the keys ascend by raw pubkey
        // bytes -- not by the order the instructions named them. That is the
        // rule most easily missed, because instruction order looks like the
        // natural one and produces a message that still parses.
        $writableSigners = self::sortKeys($meta, true, true);
        $readonlySigners = self::sortKeys($meta, true, false);
        $writableOthers = self::sortKeys($meta, false, true);
        $readonlyOthers = self::sortKeys($meta, false, false);

        // The fee payer leads, ahead of the sort rather than inside it. It is
        // a writable signer by the assignment above, so it comes out of that
        // partition and goes back at the front.
        $writableSigners = array_values(array_diff($writableSigners, [$feePayer]));
        array_unshift($writableSigners, $feePayer);

        $keys = array_merge($writableSigners, $readonlySigners, $writableOthers, $readonlyOthers);
        if (count($keys) > self::MAX_ACCOUNTS) {
            throw TxException::tooManyAccounts(count($keys));
        }
        $index = array_flip($keys);

        // The header is three counts, and they are what the partition above
        // means: everything else is derived from position.
        $message = chr(count($writableSigners) + count($readonlySigners))
            .chr(count($readonlySigners))
            .chr(count($readonlyOthers));

        $message .= self::shortVec(count($keys));
        foreach ($keys as $key) {
            $message .= self::address($key);
        }

        $message .= self::address($recentBlockhash);

        $message .= self::shortVec(count($instructions));
        foreach ($instructions as $instruction) {
            $message .= chr($index[$instruction->programId]);
            $message .= self::shortVec(count($instruction->accounts));
            foreach ($instruction->accounts as $account) {
                $message .= chr($index[$account->pubkey]);
            }
            $message .= self::shortVec(strlen($instruction->data));
            $message .= $instruction->data;
        }

        return $message;
    }

    /**
     * Frame a compiled message and its signatures into transaction bytes.
     *
     * Signatures are raw 64-byte strings -- what `sodium_crypto_sign_detached`
     * returns -- and are positional: signature `n` belongs to account key `n`,
     * which is why the fee payer leads the key list. There must be exactly as
     * many as the message's own header asks for, and this refuses rather than
     * padding, because a transaction short a signature fails at the validator
     * with nothing to say which key was missing.
     *
     * @param string[] $signatures in account-key order, the fee payer's first
     *
     * @throws TxException on a wrong number of signatures, one that is not 64
     *                     bytes, or a message too short to read its own header
     */
    public static function wire(string $message, array $signatures): string
    {
        if ($message === '') {
            throw TxException::malformedMessage();
        }

        $required = ord($message[0]);
        if (count($signatures) !== $required) {
            throw TxException::signatureCount(count($signatures), $required);
        }

        $out = self::shortVec(count($signatures));
        foreach (array_values($signatures) as $n => $signature) {
            if (strlen($signature) !== 64) {
                throw TxException::badSignature($n, strlen($signature));
            }
            $out .= $signature;
        }

        return $out.$message;
    }

    /**
     * The keys carrying these flags, ascending by raw bytes.
     *
     * Sorting the decoded bytes rather than the base58 text matters, and the
     * two differ on real key sets: base58 is not order-preserving, so sorting
     * the strings gives a different sequence and a different message. This is
     * the ordering `solana-message` gets from keeping its keys in a
     * `BTreeMap<Pubkey, _>`.
     *
     * @param array<string, array{bool, bool}> $meta
     *
     * @return string[]
     */
    private static function sortKeys(array $meta, bool $signer, bool $writable): array
    {
        $matching = [];
        foreach ($meta as $key => [$isSigner, $isWritable]) {
            if ($isSigner === $signer && $isWritable === $writable) {
                $matching[$key] = self::address((string) $key);
            }
        }
        asort($matching, SORT_STRING);

        return array_map('strval', array_keys($matching));
    }

    /** A base58 address as its 32 raw bytes. */
    private static function address(string $address): string
    {
        $raw = Base58::decode($address);
        if (strlen($raw) !== 32) {
            throw TxException::badAddress($address, strlen($raw));
        }

        return $raw;
    }

    /**
     * compact-u16: seven bits per byte, little end first, high bit set while
     * more follow. Solana's own "short vec". Lengths here are small enough
     * that this almost always emits one byte -- which is exactly why it gets
     * hardcoded as one byte by mistake, so it is written out properly.
     */
    private static function shortVec(int $n): string
    {
        $out = '';
        while (true) {
            $seven = $n & 0x7F;
            $n >>= 7;
            if ($n === 0) {
                return $out.chr($seven);
            }
            $out .= chr($seven | 0x80);
        }
    }
}
