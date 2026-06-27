//! Build the publisher allow-list the `POST /extensions` upload verifies against, from the
//! environment (lifecycle-management scope: trust is **environment, never the upload body** — an
//! attacker cannot self-trust). S7-first: production wires real publishers here; dev seeds one dev
//! publisher key. Durable storage + rotation are the deferred registry-scope open questions.
//!
//! `LB_TRUSTED_PUBKEYS` is a comma-separated `key_id=hexpubkey` list, where `hexpubkey` is the 32
//! raw Ed25519 public-key bytes as 64 lowercase hex chars (the same bytes the dev packager prints
//! for its keypair). Unset/empty → an empty allow-list (every upload `422`s — the safe default).
//! A malformed entry is skipped with a stderr warning rather than aborting boot, so one bad env line
//! cannot take the gateway down.

use lb_registry::{PublisherKey, TrustedKeys};

/// The env var naming the dev/prod publisher allow-list. One place owns the name.
pub const TRUSTED_ENV: &str = "LB_TRUSTED_PUBKEYS";

/// Parse `LB_TRUSTED_PUBKEYS` into a [`TrustedKeys`] map. Empty if unset/empty. Malformed entries are
/// logged and skipped (never panic on boot config).
pub fn trusted_from_env() -> TrustedKeys {
    match std::env::var(TRUSTED_ENV) {
        Ok(raw) if !raw.trim().is_empty() => parse(&raw),
        _ => TrustedKeys::new(),
    }
}

/// Parse a `key_id=hexpubkey,key_id2=hexpubkey2` string. Pure (no env) so it is unit-testable.
pub fn parse(raw: &str) -> TrustedKeys {
    let mut keys = TrustedKeys::new();
    for entry in raw.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        match parse_entry(entry) {
            Ok((id, key)) => {
                keys.insert(id, key);
            }
            Err(e) => eprintln!("{TRUSTED_ENV}: skipping malformed entry {entry:?}: {e}"),
        }
    }
    keys
}

/// One `key_id=hexpubkey` entry → `(id, PublisherKey)`. The hex must decode to exactly 32 bytes.
fn parse_entry(entry: &str) -> Result<(String, PublisherKey), String> {
    let (id, hex) = entry
        .split_once('=')
        .ok_or_else(|| "expected key_id=hexpubkey".to_string())?;
    let bytes = decode_hex(hex.trim())?;
    let arr: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "public key must be 32 bytes (64 hex chars)".to_string())?;
    let key = PublisherKey::from_bytes(&arr).map_err(|e| e.to_string())?;
    Ok((id.trim().to_string(), key))
}

/// Decode lowercase/uppercase hex into bytes (no dep — the registry verify idiom keeps crypto deps
/// minimal, so a 5-line hex decoder beats pulling a crate for one env parse).
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("hex length must be even".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn parses_a_valid_entry_and_skips_a_bad_one() {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk_hex = hex(&sk.verifying_key().to_bytes());
        let raw = format!("dev-publisher={pk_hex}, broken=zz, alsobad");
        let keys = parse(&raw);
        assert!(keys.contains_key("dev-publisher"), "valid entry kept");
        assert_eq!(keys.len(), 1, "the two malformed entries are skipped");
    }

    #[test]
    fn empty_input_is_an_empty_allow_list() {
        assert!(parse("").is_empty());
        assert!(parse("  ,  ").is_empty());
    }
}
