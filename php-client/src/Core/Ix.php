<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Instruction builders for the two instructions a server signs:
 * `initialize_site` and `meter_and_settle`. The payer-signed instructions
 * (`open_contract`, `renew_contract`, `close_contract`, `approve_checked`,
 * `revoke`) are deliberately absent: they're signed by a wallet adapter in
 * the browser regardless of what language the server runs, so a PHP server
 * has no use for them. See wasm-client/SPEC.md §3, "Two consumers".
 *
 * Account order and signer/writable flags mirror the `#[derive(Accounts)]`
 * structs in the on-chain program exactly, the same discipline as
 * wasm-client/src/core/ix.rs -- and this module's `meter_and_settle` output
 * is checked byte-for-byte against that crate's own output; see
 * php-client/tests/Core/IxTest.php and php-client/vectors-gen/vectors.json.
 */
final class Ix
{
    /**
     * sha256("global:<name>")[0:8], Anchor's instruction discriminator.
     * Values match wasm-client/src/core/ix.rs's `discriminator` module.
     */
    private const DISC_INITIALIZE_SITE = "\x55\x34\x80\xd0\x07\xe0\xb2\x4f";
    private const DISC_METER_AND_SETTLE = "\x8b\x11\x00\x8b\x72\xe9\x58\x79";

    /** Stand up a site's pricing. Signed by the server authority. */
    public static function initializeSite(
        Program $program,
        string $authority,
        string $mint,
        string $treasury,
        int $pagePrice,
        int $collectionThreshold,
        int $minLimit,
    ): Instruction {
        $site = Pda::siteAddress($authority, $program->id)['address'];
        $data = self::DISC_INITIALIZE_SITE
            .pack('P', $pagePrice)
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
     * Bump usage for `pageViews` and, if that carries the unpaid balance to
     * the collection threshold, transfer it. Signed by the site authority;
     * the payer is not present, and the transfer, if it happens, rides on
     * the delegate approval taken when the contract was opened or renewed.
     */
    public static function meterAndSettle(
        Program $program,
        string $site,
        string $authority,
        string $payer,
        string $payerTokenAccount,
        string $treasury,
        string $mint,
        int $pageViews,
    ): Instruction {
        $contract = Pda::contractAddress($site, $payer, $program->id)['address'];
        $data = self::DISC_METER_AND_SETTLE.pack('V', $pageViews);

        return new Instruction($program->id, [
            new AccountMeta($site, false, false),
            new AccountMeta($authority, true, false),
            new AccountMeta($payer, false, false),
            new AccountMeta($contract, false, true),
            new AccountMeta($payerTokenAccount, false, true),
            new AccountMeta($treasury, false, true),
            new AccountMeta($mint, false, false),
            new AccountMeta($program->tokenProgram, false, false),
        ], $data);
    }
}
