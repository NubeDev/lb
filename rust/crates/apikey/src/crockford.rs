//! Crockford base32 — the alphabet the key id and secret are encoded in (api-keys scope). Crockford
//! base32 is `0-9A-Z` minus the ambiguous `I`, `L`, `O`, `U`, so no field can contain a `.` or `_`
//! and the bearer grammar's dot-split is delimiter-safe. We only ever ENCODE here (random bytes → a
//! display-safe string); the auth path never decodes the secret back to bytes (it hashes the field
//! and compares constant-time), and the parse validation is a charset check, not a decode.

/// The Crockford base32 alphabet (32 symbols; `I`/`L`/`O`/`U` excluded to avoid ambiguity).
pub const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Encode `bytes` as a Crockford base32 string with no padding.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 8 + 4) / 5);
    let mut buffer: u64 = 0;
    let mut bits: u32 = 0;
    for &byte in bytes {
        buffer = (buffer << 8) | byte as u64;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            let idx = ((buffer >> bits) & 0x1f) as usize;
            out.push(ALPHABET[idx] as char);
        }
    }
    if bits > 0 {
        let idx = ((buffer << (5 - bits)) & 0x1f) as usize;
        out.push(ALPHABET[idx] as char);
    }
    out
}

/// Is `s` a non-empty string of valid Crockford base32 input symbols (case-insensitive; `I`/`L`
/// accepted as `1`, `O` as `0`, per the spec's input leniency — but `U` is not in the alphabet)?
/// Used by the bearer parser to reject malformed credentials early: a field holding a `.`, `_`,
/// space, or non-base32 char is not a key id / secret.
pub fn is_valid(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(is_symbol)
}

fn is_symbol(b: u8) -> bool {
    match b.to_ascii_uppercase() {
        b'0'..=b'9' | b'A'..=b'T' | b'V'..=b'Z' => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_uses_only_alphabet_symbols() {
        let s = encode(&[0xde, 0xad, 0xbe, 0xef, 0x42]);
        assert!(s.bytes().all(|b| ALPHABET.contains(&b)));
        assert!(is_valid(&s));
    }

    #[test]
    fn a_single_zero_byte_encodes_to_two_zero_chars() {
        assert_eq!(encode(&[0x00]), "00");
    }

    #[test]
    fn validity_rejects_delimiters_and_out_of_alphabet_chars() {
        assert!(is_valid("K7F3A"));
        assert!(is_valid("k7f3a")); // case-insensitive
        assert!(!is_valid(""));
        assert!(!is_valid("a.b"));
        assert!(!is_valid("with_underscore"));
        assert!(!is_valid("has space"));
        assert!(!is_valid("U")); // U is not Crockford
        assert!(!is_valid("u"));
        assert!(!is_valid("!@#"));
    }
}
