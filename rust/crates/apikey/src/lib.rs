//! `lb-apikey` — the pure, store-less half of the API-key credential (api-keys scope): the bearer
//! grammar, the peppered hash + constant-time compare, the high-entropy id/secret generation, and the
//! built-in role cap bundles + the list-view badge. No I/O lives here — the host `apikey` service
//! holds the store verbs + the per-request auth resolution; this crate is the logic those share, so
//! the security-critical pieces (hash, compare, parse) are unit-tested in isolation.
//!
//! The credential grammar is `lbk_{ws}.{keyid}.{secret}` — three **dot**-separated fields after the
//! `lbk_` prefix. `keyid` and `secret` are Crockford base32 (no padding, no `.`/`_`), so no field can
//! contain a `.` and parsing is a fixed split (the old `_`-delimited form collided with `_` inside
//! ids). The `{ws}.{keyid}` lets the gateway do an O(1) ws-scoped lookup with no scan.
//!
//! The hash is **`HMAC-SHA256(pepper, secret_field)`** — a keyed hash whose input is the `secret`
//! field ALONE, never the full bearer string (asserted in a unit test). High entropy (32 random
//! bytes) means a fast keyed hash is correct here, not a slow KDF; the pepper comes from
//! `lb-secrets`/env, never the DB. Comparison is constant-time (the vetted XOR-accumulate the
//! github-webhook verifier uses), never `==`.

mod crockford;
mod hash;
mod roles;
mod secret;
mod token;

pub use hash::{hash_matches, key_hash, verify_hash};
pub use roles::{
    apikey_read_caps, apikey_write_caps, badge_for_roles, ROLE_APIKEY_READ, ROLE_APIKEY_WRITE,
};
pub use secret::{generate_id, generate_secret};
pub use token::{format_bearer, parse_bearer, BearerKey};

/// The list-view display stub for a key: `lbk_{ws}.{keyid}` — everything except the secret, so the
/// admin console can identify a key without ever holding the credential.
pub fn display_prefix(ws: &str, key_id: &str) -> String {
    format!("{PREFIX}{ws}.{key_id}")
}

/// The bearer prefix every API-key credential carries.
pub const PREFIX: &str = "lbk_";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_prefix_never_includes_the_secret() {
        assert_eq!(display_prefix("acme", "k7f3a"), "lbk_acme.k7f3a");
    }
}
