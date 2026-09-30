<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Instruction builders for everything a site's server builds.
 *
 * Under the delegate design this class held only the two instructions the
 * site authority signs, because the reader's were signed by a wallet adapter
 * in the browser whatever the server ran. The fund redesign moved those onto
 * the server (SPEC §3, §4.9): it composes the one transaction the reader's
 * wallet fetches through a Solana Pay transaction request, so a PHP server
 * needs the reader's builders too. Who signs what:
 *
 * - the site authority: `initializeSite`, `meterAndSettle`;
 * - the reader's wallet: `openFund`, `deposit`, `withdraw`, `closeFund`,
 *   `openMeter`, `renewMeter`;
 * - the reader or the meter's browser key: `closeMeter`.
 *
 * Account order and signer/writable flags mirror the `#[derive(Accounts)]`
 * structs in the on-chain program exactly, the same discipline as
 * wasm-client/src/core/ix.rs -- and every builder here is checked
 * byte-for-byte against that crate's own output by conformance/vectors.php.
 * Amounts are ints (see {@see ByteReader::u64} for the ceiling); addresses
 * are base58.
 */
final class Ix
{
    /**
     * sha256("global:<name>")[0:8], Anchor's instruction discriminator.
     * Values match wasm-client/src/core/ix.rs's `discriminator` module;
     * IxTest recomputes them.
     */
    private const DISC_INITIALIZE_SITE = "\x55\x34\x80\xd0\x07\xe0\xb2\x4f";
    private const DISC_OPEN_FUND = "\x79\xe9\xcc\x1d\xe8\xed\xa6\x1e";
    private const DISC_WITHDRAW = "\xb7\x12\x46\x9c\x94\x6d\xa1\x22";
    private const DISC_CLOSE_FUND = "\xe6\xb7\x03\x70\xec\xfc\x05\xb9";
    private const DISC_OPEN_METER = "\x37\x47\x37\x7e\x26\x26\x3c\x7a";
    private const DISC_METER_AND_SETTLE = "\x8b\x11\x00\x8b\x72\xe9\x58\x79";
    private const DISC_RENEW_METER = "\xf7\xa8\x63\x6c\x13\xb7\xee\x73";
    private const DISC_CLOSE_METER = "\x66\x40\xc5\xd0\xbf\x50\x99\xa0";

    /** SPL Token's `TransferChecked` tag, a stable part of its ABI. */
    private const TAG_TRANSFER_CHECKED = "\x0c";

    /** Stand up a site's pricing. Signed by the server authority. */
    public static function initializeSite(
        Program $program,
        string $authority,
        string $mint,
        string $treasury,
        int $itemPrice,
        int $collectionThreshold,
        int $minLimit,
    ): Instruction {
        $site = Pda::siteAddress($authority, $program->id)['address'];
        $data = self::DISC_INITIALIZE_SITE
            .pack('P', $itemPrice)
            .pack('P', $collectionThreshold)
            .pack('P', $minLimit);

        return new Instruction($program->id, [
            new AccountMeta($authority, true, true),
            new AccountMeta($site, false, true),
            new AccountMeta($mint, false, false),
            new AccountMeta($treasury, false, false),
            new AccountMeta(Ids::SYSTEM_PROGRAM_ID, false, false),
        ], $data);
    }

    /**
     * Create the reader's fund at `$index`, and its token account. Must come
     * before any {@see deposit} into it in the same transaction.
     */
    public static function openFund(Program $program, string $reader, string $mint, int $index): Instruction
    {
        $fund = Pda::fundAddress($reader, $mint, $index, $program->id)['address'];

        return new Instruction($program->id, [
            new AccountMeta($reader, true, true),
            new AccountMeta($fund, false, true),
            new AccountMeta(Pda::fundTokenAccount($fund, $mint, $program->tokenProgram), false, true),
            new AccountMeta($mint, false, false),
            new AccountMeta($program->tokenProgram, false, false),
            new AccountMeta(Ids::ASSOCIATED_TOKEN_PROGRAM_ID, false, false),
            new AccountMeta(Ids::SYSTEM_PROGRAM_ID, false, false),
        ], self::DISC_OPEN_FUND.chr($index));
    }

    /**
     * Extend a fund: an SPL `transfer_checked` from `$source`, owned by
     * `$sourceOwner`, into the fund's token account. No instruction of the
     * metering program is involved; this is the transfer, addressed.
     */
    public static function deposit(
        Program $program,
        string $source,
        string $sourceOwner,
        string $fund,
        string $mint,
        int $amount,
        int $decimals,
    ): Instruction {
        return new Instruction($program->tokenProgram, [
            new AccountMeta($source, false, true),
            new AccountMeta($mint, false, false),
            new AccountMeta(Pda::fundTokenAccount($fund, $mint, $program->tokenProgram), false, true),
            new AccountMeta($sourceOwner, true, false),
        ], self::TAG_TRANSFER_CHECKED.pack('P', $amount).chr($decimals));
    }

    /** Move `$amount` out of the fund to any token account of its mint. Signed by the reader. */
    public static function withdraw(
        Program $program,
        string $reader,
        string $mint,
        int $index,
        string $destination,
        int $amount,
    ): Instruction {
        $fund = Pda::fundAddress($reader, $mint, $index, $program->id)['address'];

        return new Instruction($program->id, [
            new AccountMeta($reader, true, false),
            new AccountMeta($fund, false, false),
            new AccountMeta(Pda::fundTokenAccount($fund, $mint, $program->tokenProgram), false, true),
            new AccountMeta($destination, false, true),
            new AccountMeta($mint, false, false),
            new AccountMeta($program->tokenProgram, false, false),
        ], self::DISC_WITHDRAW.pack('P', $amount));
    }

    /** Close an empty fund with no meters open. Rent returns to the reader. */
    public static function closeFund(Program $program, string $reader, string $mint, int $index): Instruction
    {
        $fund = Pda::fundAddress($reader, $mint, $index, $program->id)['address'];

        return new Instruction($program->id, [
            new AccountMeta($reader, true, true),
            new AccountMeta($fund, false, true),
            new AccountMeta(Pda::fundTokenAccount($fund, $mint, $program->tokenProgram), false, true),
            new AccountMeta($program->tokenProgram, false, false),
        ], self::DISC_CLOSE_FUND);
    }

    /**
     * Open a meter at `$site`, drawing on `$fund`, naming the browser's
     * `$key`, a `$limit` and an `$expiry` in Unix seconds. Signed by the
     * reader, whose fund it must be; the fund's mint must be the site's.
     */
    public static function openMeter(
        Program $program,
        string $site,
        string $reader,
        string $fund,
        string $key,
        int $limit,
        int $expiry,
    ): Instruction {
        $meter = Pda::meterAddress($site, $fund, $program->id)['address'];

        return new Instruction($program->id, [
            new AccountMeta($reader, true, true),
            new AccountMeta($site, false, false),
            new AccountMeta($fund, false, true),
            new AccountMeta($meter, false, true),
            new AccountMeta(Ids::SYSTEM_PROGRAM_ID, false, false),
        ], self::DISC_OPEN_METER.Base58::decode($key).pack('P', $limit).pack('P', $expiry));
    }

    /**
     * Bump usage for `$items` and, if that carries the unpaid balance to the
     * collection threshold, transfer it from the fund. Signed by the site
     * authority; the reader is not present, and the fund's seeds authorize
     * the transfer.
     */
    public static function meterAndSettle(
        Program $program,
        string $site,
        string $authority,
        string $fund,
        string $treasury,
        string $mint,
        int $items,
    ): Instruction {
        $meter = Pda::meterAddress($site, $fund, $program->id)['address'];

        return new Instruction($program->id, [
            new AccountMeta($site, false, false),
            new AccountMeta($authority, true, false),
            new AccountMeta($fund, false, false),
            new AccountMeta($meter, false, true),
            new AccountMeta(Pda::fundTokenAccount($fund, $mint, $program->tokenProgram), false, true),
            new AccountMeta($treasury, false, true),
            new AccountMeta($mint, false, false),
            new AccountMeta($program->tokenProgram, false, false),
        ], self::DISC_METER_AND_SETTLE.pack('V', $items));
    }

    /**
     * Renew with a new limit, and a key and expiry that may be new. Naming a
     * new key is how a second device takes over the meter.
     */
    public static function renewMeter(
        Program $program,
        string $site,
        string $reader,
        string $fund,
        string $key,
        int $newLimit,
        int $expiry,
    ): Instruction {
        $meter = Pda::meterAddress($site, $fund, $program->id)['address'];

        return new Instruction($program->id, [
            new AccountMeta($reader, true, false),
            new AccountMeta($site, false, false),
            new AccountMeta($fund, false, false),
            new AccountMeta($meter, false, true),
        ], self::DISC_RENEW_METER.Base58::decode($key).pack('P', $newLimit).pack('P', $expiry));
    }

    /**
     * Close a meter. `$signer` is the reader or the meter's key; the rent
     * goes to `$reader` either way. Key-signed it is sign-out, and the site's
     * server pays the fee (SPEC §4.8).
     */
    public static function closeMeter(
        Program $program,
        string $signer,
        string $reader,
        string $site,
        string $fund,
    ): Instruction {
        $meter = Pda::meterAddress($site, $fund, $program->id)['address'];

        return new Instruction($program->id, [
            new AccountMeta($signer, true, false),
            new AccountMeta($site, false, false),
            new AccountMeta($fund, false, true),
            new AccountMeta($reader, false, true),
            new AccountMeta($meter, false, true),
        ], self::DISC_CLOSE_METER);
    }

    /**
     * Open the fund, then deposit into it from the reader's own `$source`:
     * the one ordering the program imposes, stated once. Mirrors
     * wasm-client/src/core/tx.rs.
     *
     * @return array{0: Instruction, 1: Instruction}
     */
    public static function openFundAndDeposit(
        Program $program,
        string $reader,
        string $mint,
        int $index,
        string $source,
        int $amount,
        int $decimals,
    ): array {
        $fund = Pda::fundAddress($reader, $mint, $index, $program->id)['address'];
        return [
            self::openFund($program, $reader, $mint, $index),
            self::deposit($program, $source, $reader, $fund, $mint, $amount, $decimals),
        ];
    }
}
