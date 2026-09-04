<?php

declare(strict_types=1);

namespace SolPay\Tests\Core;

use PHPUnit\Framework\TestCase;
use SolPay\Core\Base58;
use SolPay\Core\Contract;
use SolPay\Core\DecodeErrorKind;
use SolPay\Core\DecodeException;
use SolPay\Core\Mint;
use SolPay\Core\Site;
use SolPay\Core\TokenAccount;

final class StateTest extends TestCase
{
    /**
     * Genuine Anchor-serialized bytes -- pay_on_chain::state::Site's own
     * #[account] DISCRIMINATOR plus AnchorSerialize, not hand-assembled --
     * produced by php-client/vectors-gen and recorded in
     * php-client/vectors-gen/vectors.json's "site_account". Regenerate
     * with `cargo run --release > ../php/vectors.json` from vectors-gen/
     * after any change to pay-on-chain/programs/pay-on-chain/src/state.rs
     * and update this constant if it changes.
     */
    public function testSiteDecodesARealAnchorSerializedAccount(): void
    {
        $bytes = hex2bin(
            '8fff340f41a55e312c8e0047d7d6624eb2213f5d1191d37301836d6731bafa4fbe8743110bbe852aa9ab'
            .'f5f0e8b46c57452bdf96cc079830ab249020269c0c77435cc936691394a87f119555a5f2e9ab30519f511'
            .'3c79edef32f6c0d03f8cf1ded3cfad6b74333e8102700000000000090d003000000000020a10700000000'
            .'00fe',
        );

        $s = Site::decode($bytes);

        self::assertSame('3zvXTk3LUvcsusVi7pMtovKCxGtxGCjeEmGso3x91M5K', $s->authority);
        self::assertSame('CRKz4eYnALe6h4LDZwm5ZiD7cAchCb5NHQTUm9cNSaDu', $s->mint);
        self::assertSame('9Z2L3mYsKKyoH8d7o9STDSVLhvTgXsDoSfLnWSWFrksZ', $s->treasury);
        self::assertSame(10_000, $s->pagePrice);
        self::assertSame(250_000, $s->collectionThreshold);
        self::assertSame(500_000, $s->minLimit);
        self::assertSame(254, $s->bump);
    }

    /** Same discipline as {@see testSiteDecodesARealAnchorSerializedAccount}, for Contract. */
    public function testContractDecodesARealAnchorSerializedAccount(): void
    {
        $bytes = hex2bin(
            'ac8a73f27943b71af2d6713221830bf6ab79743159da843f4ea258b322eabfa5ecf92d2c8a6601f05e25'
            .'d28c5bb4ab2ef2d79b02c49f3b45ded37d11b834103065630924b770a07640420f000000000090d00300'
            .'00000000a086010000000000fd',
        );

        $c = Contract::decode($bytes);

        self::assertSame('HLwKN3khwF5WdLfbN2XsbQz8tYET3iH1eQ3HbRagG2BZ', $c->site);
        self::assertSame('7LWmtbqp9ZAiHDs2R5GVikZVRr5Zi41iqzMHodThMPa5', $c->payer);
        self::assertSame(1_000_000, $c->limit);
        self::assertSame(250_000, $c->used);
        self::assertSame(100_000, $c->paid);
        self::assertSame(253, $c->bump);
    }

    private static function siteBytes(): string
    {
        return "\x8f\xff\x34\x0f\x41\xa5\x5e\x31"
            .str_repeat("\x01", 32)
            .str_repeat("\x02", 32)
            .str_repeat("\x03", 32)
            .pack('P', 10_000)
            .pack('P', 250_000)
            .pack('P', 500_000)
            .chr(254);
    }

    public function testSiteDecodesFieldByField(): void
    {
        $s = Site::decode(self::siteBytes());

        self::assertSame(Base58::encode(str_repeat("\x01", 32)), $s->authority);
        self::assertSame(Base58::encode(str_repeat("\x02", 32)), $s->mint);
        self::assertSame(Base58::encode(str_repeat("\x03", 32)), $s->treasury);
        self::assertSame(10_000, $s->pagePrice);
        self::assertSame(250_000, $s->collectionThreshold);
        self::assertSame(500_000, $s->minLimit);
        self::assertSame(254, $s->bump);
    }

    public function testAWrongDiscriminatorIsRefused(): void
    {
        $bytes = self::siteBytes();
        $bytes[0] = chr(ord($bytes[0]) ^ 0xFF);

        try {
            Site::decode($bytes);
            self::fail('expected a DecodeException');
        } catch (DecodeException $e) {
            self::assertSame(DecodeErrorKind::WrongDiscriminator, $e->kind);
        }
    }

    public function testAShortAccountIsRefusedBeforeItIsRead(): void
    {
        $bytes = substr(self::siteBytes(), 0, -1);

        try {
            Site::decode($bytes);
            self::fail('expected a DecodeException');
        } catch (DecodeException $e) {
            self::assertSame(DecodeErrorKind::WrongLength, $e->kind);
            self::assertSame(129, $e->expectedLength);
            self::assertSame(128, $e->actualLength);
        }
    }

    public function testUnpaidAndOutstandingSaturate(): void
    {
        $c = new Contract(
            site: Base58::encode(str_repeat("\x00", 32)),
            payer: Base58::encode(str_repeat("\x00", 32)),
            limit: 100,
            used: 40,
            paid: 60, // impossible on chain; the helpers must not go negative
            bump: 255,
        );

        self::assertSame(0, $c->unpaid());
        self::assertSame(40, $c->outstanding());
    }

    public function testTokenAccountDecodesAmountAndDelegation(): void
    {
        // Full 165-byte SPL layout, zero-padded; the decoder never reads
        // past delegated_amount (offset 121..129), but the length check
        // requires the real minimum -- close_authority and its tag live in
        // the untouched tail, same as the Rust test this mirrors.
        $bytes = str_repeat("\x00", 165);
        $bytes = substr_replace($bytes, str_repeat("\x04", 32), 0, 32);   // mint
        $bytes = substr_replace($bytes, str_repeat("\x05", 32), 32, 32);  // owner
        $bytes = substr_replace($bytes, pack('P', 900), 64, 8);           // amount
        $bytes = substr_replace($bytes, pack('V', 1), 72, 4);             // delegate tag: Some
        $bytes = substr_replace($bytes, str_repeat("\x06", 32), 76, 32);  // delegate pubkey
        $bytes = substr_replace($bytes, pack('P', 400), 121, 8);          // delegated_amount
        self::assertSame(165, strlen($bytes));

        $t = TokenAccount::decode($bytes);
        self::assertSame(Base58::encode(str_repeat("\x04", 32)), $t->mint);
        self::assertSame(Base58::encode(str_repeat("\x05", 32)), $t->owner);
        self::assertSame(900, $t->amount);
        self::assertSame(Base58::encode(str_repeat("\x06", 32)), $t->delegate);
        self::assertSame(400, $t->delegatedAmount);

        // Tag zero means no delegate, whatever bytes follow it.
        $noDelegate = substr_replace($bytes, pack('V', 0), 72, 4);
        self::assertNull(TokenAccount::decode($noDelegate)->delegate);
    }

    public function testTokenAccountRejectsAShortAccount(): void
    {
        $this->expectException(DecodeException::class);
        TokenAccount::decode(str_repeat("\x00", 100));
    }

    public function testMintDecimalsReadsTheFixedOffset(): void
    {
        $mint = str_repeat("\x00", 82);
        $mint[44] = chr(6);
        self::assertSame(6, Mint::decimals($mint));

        // Token-2022 mints carry extensions past the base layout.
        $extended = $mint.str_repeat("\x07", 40);
        self::assertSame(6, Mint::decimals($extended));
    }

    public function testMintDecimalsRejectsAShortAccount(): void
    {
        $this->expectException(DecodeException::class);
        Mint::decimals(str_repeat("\x00", 10));
    }
}
