//! Decoding the program's accounts.
//!
//! A site's UI cannot render anything about a contract without these: the
//! limit, the usage and the price all live in account data. Anchor writes an
//! 8-byte discriminator followed by borsh, and these read that back by hand.
//!
//! By hand deliberately. A `BorshDeserialize` derive needs the `borsh` feature
//! on `solana-pubkey`, which turns on *both* borsh generations and pulls
//! eleven packages into the tree, including a second `syn`. This crate already
//! hand-rolls two SPL Token instructions rather than depend on `spl-token`,
//! for the same reason: what ships in the bundle is worth forty lines.
//!
//! The field order below is load-bearing and invisible at a glance, so it is
//! pinned by tests in `pay-on-chain/tests`, the one place this crate and the
//! program build together: Anchor serializes an account there and this code
//! reads it back. A field added or moved on chain fails that test rather than
//! silently shifting every field after it.

use solana_pubkey::Pubkey;

/// Anchor account discriminators: the first eight bytes of
/// `sha256("account:<StructName>")`. Precomputed for the same reason the
/// instruction discriminators are; the parity tests recompute them.
pub mod discriminator {
    pub const SITE: [u8; 8] = [143, 255, 52, 15, 65, 165, 94, 49];
    pub const CONTRACT: [u8; 8] = [172, 138, 115, 242, 121, 67, 183, 26];
}

/// 8 discriminator + 32 + 32 + 32 + 8 + 8 + 8 + 1
pub const SITE_LEN: usize = 129;
/// 8 discriminator + 32 + 32 + 8 + 8 + 8 + 1
pub const CONTRACT_LEN: usize = 97;
/// SPL mint accounts are at least this long; Token-2022 adds extensions after.
pub const MINT_MIN_LEN: usize = 82;
/// SPL token accounts are at least this long, same caveat.
pub const TOKEN_ACCOUNT_LEN: usize = 165;
/// Byte offset of `decimals` within an SPL mint.
const MINT_DECIMALS_OFFSET: usize = 44;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The account belongs to something else, or to a later layout.
    WrongDiscriminator,
    WrongLength { expected: usize, got: usize },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::WrongDiscriminator => {
                write!(f, "account discriminator does not match")
            }
            DecodeError::WrongLength { expected, got } => {
                write!(f, "account should be {expected} bytes, got {got}")
            }
        }
    }
}

/// Per-site configuration: pricing, the mint, and who may meter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Site {
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub treasury: Pubkey,
    pub page_price: u64,
    pub collection_threshold: u64,
    pub min_limit: u64,
    pub bump: u8,
}

/// One payer's spending contract with one site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Contract {
    pub site: Pubkey,
    pub payer: Pubkey,
    pub limit: u64,
    pub used: u64,
    pub paid: u64,
    pub bump: u8,
}

impl Contract {
    /// Usage accrued but not yet transferred. Mirrors the program.
    pub fn unpaid(&self) -> u64 {
        self.used.saturating_sub(self.paid)
    }

    /// What the delegate allowance still has to cover under the current limit.
    pub fn outstanding(&self) -> u64 {
        self.limit.saturating_sub(self.paid)
    }
}

/// Length first, then discriminator. Checking the length up front is what
/// makes every read below safe to slice without checking again.
fn check(data: &[u8], want: [u8; 8], len: usize) -> Result<(), DecodeError> {
    if data.len() != len {
        return Err(DecodeError::WrongLength {
            expected: len,
            got: data.len(),
        });
    }
    if data[..8] != want {
        return Err(DecodeError::WrongDiscriminator);
    }
    Ok(())
}

/// Walks a fixed layout, starting past the discriminator.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    /// `at` is where the fields start: 8 for an Anchor account, 0 for an SPL
    /// one, which carries no discriminator.
    fn new(bytes: &'a [u8], at: usize) -> Self {
        Reader { bytes, at }
    }

    /// A 4-byte-tagged optional pubkey, the way SPL Token writes `COption`.
    fn coption_pubkey(&mut self) -> Option<Pubkey> {
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&self.bytes[self.at..self.at + 4]);
        self.at += 4;
        let key = self.pubkey();
        if u32::from_le_bytes(tag) == 1 {
            Some(key)
        } else {
            None
        }
    }

    fn skip(&mut self, n: usize) {
        self.at += n;
    }

    fn pubkey(&mut self) -> Pubkey {
        let mut buf = [0u8; 32];
        buf.copy_from_slice(&self.bytes[self.at..self.at + 32]);
        self.at += 32;
        Pubkey::new_from_array(buf)
    }

    fn u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.bytes[self.at..self.at + 8]);
        self.at += 8;
        u64::from_le_bytes(buf)
    }

    fn u8(&mut self) -> u8 {
        let b = self.bytes[self.at];
        self.at += 1;
        b
    }
}

impl Site {
    pub fn decode(data: &[u8]) -> Result<Self, DecodeError> {
        check(data, discriminator::SITE, SITE_LEN)?;
        let mut r = Reader::new(data, 8);
        Ok(Site {
            authority: r.pubkey(),
            mint: r.pubkey(),
            treasury: r.pubkey(),
            page_price: r.u64(),
            collection_threshold: r.u64(),
            min_limit: r.u64(),
            bump: r.u8(),
        })
    }
}

impl Contract {
    pub fn decode(data: &[u8]) -> Result<Self, DecodeError> {
        check(data, discriminator::CONTRACT, CONTRACT_LEN)?;
        let mut r = Reader::new(data, 8);
        Ok(Contract {
            site: r.pubkey(),
            payer: r.pubkey(),
            limit: r.u64(),
            used: r.u64(),
            paid: r.u64(),
            bump: r.u8(),
        })
    }
}

/// The mint's decimals, one byte at a fixed offset.
///
/// `approve_checked` needs this and there is nowhere else to get it without
/// decoding a mint by hand. Accepts anything at least mint-sized, so a
/// Token-2022 mint carrying extensions decodes too.
pub fn mint_decimals(mint_account_data: &[u8]) -> Result<u8, DecodeError> {
    if mint_account_data.len() < MINT_MIN_LEN {
        return Err(DecodeError::WrongLength {
            expected: MINT_MIN_LEN,
            got: mint_account_data.len(),
        });
    }
    Ok(mint_account_data[MINT_DECIMALS_OFFSET])
}


/// The payer's SPL token account, as much of it as this crate needs.
///
/// Not an Anchor account, so no discriminator: SPL writes a fixed 165-byte
/// layout. Token-2022 appends extensions past that, which is why anything at
/// least that long decodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenAccount {
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub amount: u64,
    /// Whom the owner has approved, if anyone. SPL clears this when the
    /// approved amount reaches zero, so its absence is ordinary rather than
    /// exceptional.
    pub delegate: Option<Pubkey>,
    /// How much that delegate may still move. Decremented by every delegated
    /// transfer, which is why it is not simply the limit.
    pub delegated_amount: u64,
}

impl TokenAccount {
    pub fn decode(data: &[u8]) -> Result<Self, DecodeError> {
        if data.len() < TOKEN_ACCOUNT_LEN {
            return Err(DecodeError::WrongLength {
                expected: TOKEN_ACCOUNT_LEN,
                got: data.len(),
            });
        }
        let mut r = Reader::new(data, 0);
        let mint = r.pubkey();
        let owner = r.pubkey();
        let amount = r.u64();
        let delegate = r.coption_pubkey();
        r.skip(1); // state
        r.skip(12); // is_native: COption<u64>
        let delegated_amount = r.u64();
        Ok(TokenAccount {
            mint,
            owner,
            amount,
            delegate,
            delegated_amount,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site_bytes() -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&discriminator::SITE);
        v.extend_from_slice(&[1u8; 32]);
        v.extend_from_slice(&[2u8; 32]);
        v.extend_from_slice(&[3u8; 32]);
        v.extend_from_slice(&10_000u64.to_le_bytes());
        v.extend_from_slice(&250_000u64.to_le_bytes());
        v.extend_from_slice(&500_000u64.to_le_bytes());
        v.push(254);
        v
    }

    #[test]
    fn site_decodes_field_by_field() {
        let s = Site::decode(&site_bytes()).unwrap();
        assert_eq!(s.authority, Pubkey::new_from_array([1u8; 32]));
        assert_eq!(s.mint, Pubkey::new_from_array([2u8; 32]));
        assert_eq!(s.treasury, Pubkey::new_from_array([3u8; 32]));
        assert_eq!(s.page_price, 10_000);
        assert_eq!(s.collection_threshold, 250_000);
        assert_eq!(s.min_limit, 500_000);
        assert_eq!(s.bump, 254);
    }

    #[test]
    fn a_wrong_discriminator_is_refused() {
        let mut bytes = site_bytes();
        bytes[0] ^= 0xff;
        assert_eq!(Site::decode(&bytes), Err(DecodeError::WrongDiscriminator));
    }

    #[test]
    fn a_short_account_is_refused_before_it_is_read() {
        let bytes = site_bytes();
        assert_eq!(
            Site::decode(&bytes[..SITE_LEN - 1]),
            Err(DecodeError::WrongLength {
                expected: SITE_LEN,
                got: SITE_LEN - 1
            })
        );
    }

    #[test]
    fn unpaid_and_outstanding_saturate() {
        let c = Contract {
            site: Pubkey::new_from_array([0u8; 32]),
            payer: Pubkey::new_from_array([0u8; 32]),
            limit: 100,
            used: 40,
            paid: 60, // impossible on chain; the helpers must not panic
            bump: 255,
        };
        assert_eq!(c.unpaid(), 0);
        assert_eq!(c.outstanding(), 40);
    }

    #[test]
    fn token_account_decodes_amount_and_delegation() {
        let mut a = vec![0u8; TOKEN_ACCOUNT_LEN];
        a[0..32].copy_from_slice(&[4u8; 32]); // mint
        a[32..64].copy_from_slice(&[5u8; 32]); // owner
        a[64..72].copy_from_slice(&900u64.to_le_bytes()); // amount
        a[72..76].copy_from_slice(&1u32.to_le_bytes()); // delegate: Some
        a[76..108].copy_from_slice(&[6u8; 32]);
        a[121..129].copy_from_slice(&400u64.to_le_bytes()); // delegated_amount

        let t = TokenAccount::decode(&a).unwrap();
        assert_eq!(t.mint, Pubkey::new_from_array([4u8; 32]));
        assert_eq!(t.owner, Pubkey::new_from_array([5u8; 32]));
        assert_eq!(t.amount, 900);
        assert_eq!(t.delegate, Some(Pubkey::new_from_array([6u8; 32])));
        assert_eq!(t.delegated_amount, 400);

        // Tag zero means no delegate, whatever bytes follow it.
        a[72..76].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(TokenAccount::decode(&a).unwrap().delegate, None);

        assert!(TokenAccount::decode(&a[..100]).is_err());
    }

    #[test]
    fn mint_decimals_reads_the_fixed_offset() {
        let mut mint = vec![0u8; MINT_MIN_LEN];
        mint[MINT_DECIMALS_OFFSET] = 6;
        assert_eq!(mint_decimals(&mint).unwrap(), 6);

        // Token-2022 mints carry extensions past the base layout.
        let mut extended = mint.clone();
        extended.extend_from_slice(&[7u8; 40]);
        assert_eq!(mint_decimals(&extended).unwrap(), 6);

        assert!(mint_decimals(&mint[..10]).is_err());
    }
}
