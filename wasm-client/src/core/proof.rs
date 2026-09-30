//! Key proof (SPEC §6.6): is the browser in front of a site the one a meter
//! names?
//!
//! The site's server composes some bytes, the page signs them with the key
//! it generated (§4.8), and the server checks the signature against the key
//! the meter names. There is no message format -- the bytes are the site's
//! own and nobody else interprets them -- and no address to recover, because
//! the key is fetched from the chain.
//!
//! Shipped here, where Sign In With Solana verification was deliberately not
//! (SPEC §6.6 has both arguments): what this implements is Ed25519, which has
//! one definition and test vectors in RFC 8032, not a byte-exact format two
//! libraries could read differently. A proof verified wrongly admits anyone,
//! so the check is too important to leave to a paragraph.
//!
//! **A valid signature is not a live meter.** After this says yes the site
//! still owes three checks, which are its own and not this function's: that
//! the meter's `key` is the key it verified against, that the meter is not
//! `expired(now)`, and that the meter's `site` is this site. And the bytes
//! must carry a nonce and a time the server issued and remembers, or a
//! replayed proof passes forever.

use ed25519_dalek::{Signature, VerifyingKey};

/// True when `signature` is a valid Ed25519 signature by `key` over
/// `message`. Nothing else: no format, no expiry, no nonce -- those are the
/// site's.
///
/// Strict verification: a key of small order or a signature with a
/// non-canonical scalar is refused, which is what libsodium's
/// `crypto_sign_verify_detached` does too, so the PHP port agrees.
pub fn verify_key(key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let Ok(key) = VerifyingKey::from_bytes(key) else {
        return false;
    };
    key.verify_strict(message, &Signature::from_bytes(signature)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex<const N: usize>(s: &str) -> [u8; N] {
        let v: Vec<u8> = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect();
        v.try_into().unwrap()
    }

    fn unhex_vec(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// RFC 8032 §7.1, tests 1 to 3.
    const RFC_8032: [(&str, &str, &str); 3] = [
        (
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
            "",
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        ),
        (
            "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
            "72",
            "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
        ),
        (
            "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
            "af82",
            "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
        ),
    ];

    #[test]
    fn the_rfc_8032_vectors_verify() {
        for (key, message, signature) in RFC_8032 {
            assert!(verify_key(&unhex(key), &unhex_vec(message), &unhex(signature)));
        }
    }

    #[test]
    fn anything_changed_does_not() {
        let (key, message, signature) = RFC_8032[2];
        let (key, message, signature): ([u8; 32], Vec<u8>, [u8; 64]) =
            (unhex(key), unhex_vec(message), unhex(signature));

        let mut other = message.clone();
        other[0] ^= 1;
        assert!(!verify_key(&key, &other, &signature), "another message");

        let mut bad = signature;
        bad[0] ^= 1;
        assert!(!verify_key(&key, &message, &bad), "another signature");

        let (wrong_key, _, _) = RFC_8032[1];
        assert!(!verify_key(&unhex(wrong_key), &message, &signature), "another key");

        // Not a point at all: refused, not a panic.
        assert!(!verify_key(&[0xff; 32], &message, &signature));
    }
}
