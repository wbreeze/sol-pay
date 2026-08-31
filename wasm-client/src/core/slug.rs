//! Bump slugs.
//!
//! A slug is 16 raw bytes on chain and a URL-safe string in a path. The
//! encoding here is base64url without padding, which fits 16 bytes into 22
//! characters and survives a URL untouched.
//!
//! Randomness is deliberately *not* generated here. The caller supplies the
//! bytes, so a browser can use `crypto.getRandomValues` and a server can use
//! its own CSPRNG, and this crate needs no `getrandom` backend to build for
//! `wasm32-unknown-unknown`.

pub const SLUG_LEN: usize = 16;

const ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slug(pub [u8; SLUG_LEN]);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlugError {
    WrongLength(usize),
    BadCharacter(char),
}

impl core::fmt::Display for SlugError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SlugError::WrongLength(n) => write!(f, "slug must be 22 characters, got {n}"),
            SlugError::BadCharacter(c) => write!(f, "slug contains invalid character {c:?}"),
        }
    }
}

impl Slug {
    /// Build from caller-supplied random bytes.
    pub fn from_bytes(bytes: [u8; SLUG_LEN]) -> Self {
        Slug(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; SLUG_LEN] {
        &self.0
    }

    /// base64url, no padding. 16 bytes -> 22 characters.
    pub fn encode(&self) -> String {
        let mut out = String::with_capacity(22);
        let mut acc: u32 = 0;
        let mut bits = 0u32;
        for &b in self.0.iter() {
            acc = (acc << 8) | b as u32;
            bits += 8;
            while bits >= 6 {
                bits -= 6;
                out.push(ALPHABET[((acc >> bits) & 0x3f) as usize] as char);
            }
        }
        if bits > 0 {
            out.push(ALPHABET[((acc << (6 - bits)) & 0x3f) as usize] as char);
        }
        out
    }

    pub fn parse(s: &str) -> Result<Self, SlugError> {
        if s.len() != 22 {
            return Err(SlugError::WrongLength(s.len()));
        }
        let mut bytes = [0u8; SLUG_LEN];
        let mut acc: u32 = 0;
        let mut bits = 0u32;
        let mut i = 0usize;
        for c in s.chars() {
            let v = ALPHABET
                .iter()
                .position(|&a| a as char == c)
                .ok_or(SlugError::BadCharacter(c))? as u32;
            acc = (acc << 6) | v;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                if i < SLUG_LEN {
                    bytes[i] = ((acc >> bits) & 0xff) as u8;
                    i += 1;
                }
            }
        }
        Ok(Slug(bytes))
    }
}

impl core::fmt::Display for Slug {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.encode())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let bytes = [
            0u8, 1, 2, 3, 250, 251, 252, 253, 128, 64, 32, 16, 8, 4, 2, 255,
        ];
        let slug = Slug::from_bytes(bytes);
        let s = slug.encode();
        assert_eq!(s.len(), 22);
        assert_eq!(Slug::parse(&s).unwrap(), slug);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(matches!(Slug::parse("tooshort"), Err(SlugError::WrongLength(8))));
        let bad = "A".repeat(21) + "*";
        assert!(matches!(Slug::parse(&bad), Err(SlugError::BadCharacter('*'))));
    }
}
