use anyhow::{bail, Result};

/// Lowercase hex is the on-disk publisher-key format because it is stable, pasteable, and matches
/// the `LB_TRUSTED_PUBKEYS` public-key shape documented for the existing `lb-pack` flow.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}

pub(crate) fn decode(s: &str) -> Result<Vec<u8>> {
    // Reject malformed key material before `ed25519-dalek` sees it; a bad publisher seed should fail
    // at the custody boundary, not later while constructing an artifact.
    if !s.len().is_multiple_of(2) {
        bail!("hex length must be even");
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(Into::into))
        .collect()
}
