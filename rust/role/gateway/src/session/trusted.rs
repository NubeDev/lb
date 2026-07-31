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
//!
//! ## The development escape hatch: `LB_EXT_UNTRUSTED_KEY`
//!
//! The allow-list gate is **no longer unconditional**. `LB_EXT_UNTRUSTED_KEY=allow` puts the node in
//! [`lb_registry::Authenticity::WaivedUntrustedKey`]: an artifact signed by a key that is *not* in
//! `LB_TRUSTED_PUBKEYS` (or not validly signed at all) is accepted. This exists because keeping
//! `LB_TRUSTED_PUBKEYS` in sync across bench nodes is real toil — a regenerated dev key means every
//! publish `422`s until someone edits a unit file and re-applies.
//!
//! What it does **not** do: waive content integrity. The digest is still recomputed and compared, so
//! a corrupt or truncated upload is still rejected. See `lb_registry::verify` — the two checks are
//! kept structurally separate precisely so this hatch cannot reach the first one.
//!
//! Three deliberate properties, in tension with the rest of this module's conventions:
//!
//! 1. **Exact-token, not presence.** Unlike `LB_DEV_LOGIN`/`LB_BROWSER_SESSION_SECURE`, which are
//!    presence-flags, this knob requires the literal value `allow`. Presence semantics would be
//!    actively hazardous here: an operator writing `LB_EXT_UNTRUSTED_KEY=off` or `=0` intending to
//!    *keep* the gate would silently disable it. Breaking the convention is the safe choice.
//! 2. **Fail closed on anything else.** Unset, empty, misspelled, `true`, `off`, `ALLOW` — every one
//!    of these means [`Authenticity::Required`], i.e. today's behaviour exactly. There is no new
//!    default-permissive path. An unparseable value is logged and treated as **on**, mirroring the
//!    `parse` idiom below where a malformed entry is skipped rather than aborting boot.
//! 3. **Loud.** [`authenticity_from_env`] warns on stderr at boot when it returns the waiver, the
//!    host layer warns on every waived artifact, and the gateway reports it on `GET /health` so an
//!    operator who inherits a box can see it without reading the unit file.

use lb_registry::{Authenticity, PublisherKey, TrustedKeys};

/// The env var naming the dev/prod publisher allow-list. One place owns the name.
pub const TRUSTED_ENV: &str = "LB_TRUSTED_PUBKEYS";

/// The env var that waives the publisher-signature check. `LB_EXT_*` is the existing prefix family
/// for extension-scoped knobs (`LB_EXT_ID`, `LB_EXT_TOKEN`, `LB_EXT_UI_DIR`). Named for the precise
/// thing it permits — an *untrusted key*, not an unsigned artifact (the artifact is still signed;
/// it is the signer that goes unchecked) and not a general "trust gate", which would wrongly imply
/// the digest check is in scope too.
pub const UNTRUSTED_KEY_ENV: &str = "LB_EXT_UNTRUSTED_KEY";

/// The one value that disables the check. Spelled out rather than a bool so the intent is explicit
/// in a unit file and greppable across a fleet.
pub const UNTRUSTED_KEY_ALLOW: &str = "allow";

/// Parse `LB_TRUSTED_PUBKEYS` into a [`TrustedKeys`] map. Empty if unset/empty. Malformed entries are
/// logged and skipped (never panic on boot config).
pub fn trusted_from_env() -> TrustedKeys {
    match std::env::var(TRUSTED_ENV) {
        Ok(raw) if !raw.trim().is_empty() => parse(&raw),
        _ => TrustedKeys::new(),
    }
}

/// Read [`UNTRUSTED_KEY_ENV`] into an [`Authenticity`], **failing closed on everything but the exact
/// disable token**, and warn loudly on stderr when the gate comes up disabled.
///
/// The boot warning is one of the three loud surfaces (see the module docs). It is the cheapest one
/// and the least reliable — a journal rotates — which is why it is not the only one.
pub fn authenticity_from_env() -> Authenticity {
    match std::env::var(UNTRUSTED_KEY_ENV) {
        Ok(raw) => authenticity_from_value(&raw),
        // Unset is the overwhelmingly common case and the safe one: say nothing, enforce the gate.
        Err(_) => Authenticity::Required,
    }
}

/// Map one env value onto an [`Authenticity`]. Pure (no env) so it is unit-testable, matching
/// [`parse`]'s split.
///
/// Only the exact token [`UNTRUSTED_KEY_ALLOW`] disables the check — case-sensitively, after trimming
/// surrounding whitespace (a unit file `Environment=` line picks up strays; that is a formatting
/// artifact, not a different intent). Everything else, including a plausible-looking `true`/`1`/`off`,
/// keeps the gate on and is reported as ignored rather than silently obeyed in either direction.
pub fn authenticity_from_value(raw: &str) -> Authenticity {
    let value = raw.trim();
    if value == UNTRUSTED_KEY_ALLOW {
        eprintln!(
            "WARNING: {UNTRUSTED_KEY_ENV}={UNTRUSTED_KEY_ALLOW} — the extension publisher trust \
             gate is DISABLED. This node accepts extensions signed by ANY key, including keys not \
             in {TRUSTED_ENV}, and will pull and run their code. Content integrity is still \
             verified (a corrupt artifact is still rejected), but authorship is NOT. This is a \
             development-only setting: if you did not deliberately set it on a bench node, unset it \
             and restart. Reported as \"waived\" on GET /health."
        );
        return Authenticity::WaivedUntrustedKey;
    }
    if !value.is_empty() {
        // Fail closed and SAY SO. A silent fallback here is the trap: an operator who typed
        // `=true` expecting the hatch would otherwise get today's 422s with no clue why.
        eprintln!(
            "{UNTRUSTED_KEY_ENV}: ignoring unrecognised value {value:?} — the publisher trust gate \
             stays ENABLED. The only value that disables it is {UNTRUSTED_KEY_ALLOW:?}."
        );
    }
    Authenticity::Required
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

    // ---- `LB_EXT_UNTRUSTED_KEY` — the escape hatch must be hard to turn on by accident. -------

    #[test]
    fn the_exact_token_disables_the_gate() {
        assert_eq!(
            authenticity_from_value("allow"),
            Authenticity::WaivedUntrustedKey
        );
        // Surrounding whitespace is a unit-file formatting artifact, not a different intent.
        assert_eq!(
            authenticity_from_value("  allow\n"),
            Authenticity::WaivedUntrustedKey
        );
    }

    #[test]
    fn a_garbage_value_leaves_the_gate_on() {
        // The load-bearing fail-closed test. Every one of these is something an operator might
        // plausibly type; none of them may disable the check.
        for raw in [
            "",
            " ",
            "1",
            "0",
            "true",
            "false",
            "on",
            "off",
            "yes",
            "no",
            "ALLOW",
            "Allow",
            "allowed",
            "allow=true",
            "disable",
            "insecure",
            "please",
        ] {
            assert_eq!(
                authenticity_from_value(raw),
                Authenticity::Required,
                "{raw:?} must NOT disable the publisher trust gate"
            );
        }
    }

    #[test]
    fn required_is_the_default_posture() {
        // Absent env → today's behaviour exactly, with no new default-permissive path.
        assert_eq!(Authenticity::default(), Authenticity::Required);
        assert!(!Authenticity::Required.is_waived());
        assert!(Authenticity::WaivedUntrustedKey.is_waived());
    }
}
