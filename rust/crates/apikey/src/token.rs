//! The bearer credential grammar: `lbk_{ws}.{keyid}.{secret}` — three **dot**-separated fields
//! after the `lbk_` prefix (api-keys scope). `keyid` and `secret` are Crockford base32 (no `.`/`_`),
//! so no field can contain a `.` and parsing is a fixed split (the old `_`-delimited form collided
//! with `_` inside ids). The `{ws}.{keyid}` lets the gateway do an O(1) ws-scoped lookup with no
//! scan.

use crate::crockford::is_valid;
use crate::PREFIX;

/// A parsed bearer credential: the workspace it scopes, the key id, and the raw secret field. The
/// secret leaves the host exactly once (at create); it is present here only on the gateway's auth
/// path, where it is hashed and compared constant-time, never logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BearerKey<'a> {
    pub ws: &'a str,
    pub key_id: &'a str,
    pub secret: &'a str,
}

/// Parse a bearer string into its three fields, or `None` if it is not a well-formed API-key
/// credential (wrong prefix, wrong field count, an empty field, or an id/secret field holding a
/// non-base32 char). Returns references into `s` — no allocation on the hot path.
///
/// Splitting on `.` and requiring exactly three fields rejects any field that itself contains a `.`
/// (it would yield more than three parts), so the grammar is delimiter-safe by construction.
pub fn parse_bearer(s: &str) -> Option<BearerKey<'_>> {
    let rest = s.strip_prefix(PREFIX)?;
    let fields: Vec<&str> = rest.split('.').collect();
    if fields.len() != 3 {
        return None;
    }
    let ws = fields[0];
    let key_id = fields[1];
    let secret = fields[2];
    if ws.is_empty() || !is_valid(key_id) || !is_valid(secret) {
        return None;
    }
    Some(BearerKey { ws, key_id, secret })
}

/// Format the three fields back into the bearer string (the value returned once at create).
pub fn format_bearer(ws: &str, key_id: &str, secret: &str) -> String {
    format!("{PREFIX}{ws}.{key_id}.{secret}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_credential() {
        let b = parse_bearer("lbk_acme.k7f3a.s3cr3tfield").unwrap();
        assert_eq!(b.ws, "acme");
        assert_eq!(b.key_id, "k7f3a");
        assert_eq!(b.secret, "s3cr3tfield");
    }

    #[test]
    fn format_then_parse_round_trips() {
        let bearer = format_bearer("acme", "k7f3a", "ABCDEF23");
        assert_eq!(bearer, "lbk_acme.k7f3a.ABCDEF23");
        let b = parse_bearer(&bearer).unwrap();
        assert_eq!((b.ws, b.key_id, b.secret), ("acme", "k7f3a", "ABCDEF23"));
    }

    #[test]
    fn rejects_wrong_prefix() {
        assert_eq!(parse_bearer("lb_acme.k7f3a.secret"), None);
        assert_eq!(parse_bearer("acme.k7f3a.secret"), None);
        assert_eq!(
            parse_bearer("lbk_acme.k7f3a.secret".strip_prefix("lbk").unwrap()),
            None
        );
    }

    #[test]
    fn rejects_wrong_field_count() {
        // Too few fields.
        assert_eq!(parse_bearer("lbk_acme.k7f3a"), None);
        // A dot inside what should be one field → four parts.
        assert_eq!(parse_bearer("lbk_acme.k7f3a.sec.ret"), None);
        // A dot inside the ws → four parts.
        assert_eq!(parse_bearer("lbk_ac.me.k7f3a.secret"), None);
        // Too many fields outright.
        assert_eq!(parse_bearer("lbk_a.b.c.d"), None);
    }

    #[test]
    fn rejects_empty_fields_and_non_base32() {
        assert_eq!(parse_bearer("lbk_.k7f3a.secret"), None); // empty ws
        assert_eq!(parse_bearer("lbk_acme..secret"), None); // empty id
        assert_eq!(parse_bearer("lbk_acme.k7f3a."), None); // empty secret
                                                           // Non-base32 chars in the id/secret.
        assert_eq!(parse_bearer("lbk_acme.k7_3a.secret"), None);
        assert_eq!(parse_bearer("lbk_acme.k7f3a.bad!char"), None);
    }
}
