//! Compute an artifact's content digest — SHA-256 over the manifest AND the wasm bytes, with a
//! length-prefixed framing so the two fields cannot be slid past each other (registry scope, the
//! "digest must bind manifest *and* bytes" risk).
//!
//! Signing only the wasm would let a tampered manifest (e.g. an inflated `capabilities.request`)
//! ride a valid signature; signing the bare concatenation would let bytes move across the
//! manifest/wasm boundary undetected. Framing each field with its length closes both doors: the
//! digest commits to exactly `(manifest_toml, wasm)` as a pair.

use sha2::{Digest, Sha256};

/// The 32-byte SHA-256 content digest binding `manifest_toml` and `wasm`. Deterministic: the same
/// inputs always produce the same digest (no salt, no wall-clock) — what the publisher signs and
/// what the cache is keyed by. Framing: `len(manifest) ‖ manifest ‖ len(wasm) ‖ wasm`, lengths as
/// 8-byte big-endian, so a byte cannot move between the two fields without changing the digest.
pub fn digest(manifest_toml: &str, wasm: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update((manifest_toml.len() as u64).to_be_bytes());
    hasher.update(manifest_toml.as_bytes());
    hasher.update((wasm.len() as u64).to_be_bytes());
    hasher.update(wasm);
    hasher.finalize().into()
}

/// Lowercase-hex of the digest — the stable, printable cache key (`cached:{digest_hex}`) and the
/// value an artifact/catalog record carries. Hex (not base64) so it is a safe SurrealDB id segment.
pub fn digest_hex(digest: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in digest {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest_is_deterministic() {
        let a = digest("id = \"hello\"", b"\0asm\x01");
        let b = digest("id = \"hello\"", b"\0asm\x01");
        assert_eq!(a, b);
        assert_eq!(digest_hex(&a).len(), 64);
    }

    #[test]
    fn framing_binds_the_boundary() {
        // Without length-prefixing, ("ab","c") and ("a","bc") would hash equal. Framing separates them.
        assert_ne!(digest("ab", b"c"), digest("a", b"bc"));
    }

    #[test]
    fn a_changed_byte_changes_the_digest() {
        assert_ne!(digest("manifest", b"wasm"), digest("manifest", b"wasn"));
        assert_ne!(digest("manifest", b"wasm"), digest("manifeyt", b"wasm"));
    }
}
