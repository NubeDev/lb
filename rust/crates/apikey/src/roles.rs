//! The two built-in roles an API key is minted under (api-keys scope), and the list-view badge
//! derived from a key's assigned roles. A key's permissions are just grants on `Subject::Key` —
//! read-only vs read-write and tool/page limits are *which caps the key resolves to*, enforced at
//! the existing chokepoint. These roles ship the common case in one click; finer custom caps are an
//! ordinary grant on `key:{id}`.
//!
//! The write role deliberately grants **action-named** tool-call wildcards (`*.write`, `*.create`,
//! …), NOT `mcp:*.*:call`: a blanket `*.*` would match the management resource `apikey.manage` and
//! empower a data key to mint/revoke other keys. The action-named set covers the data-plane write
//! verbs a read-write appliance needs, without crossing into key administration.

/// The built-in read-only role name.
pub const ROLE_APIKEY_READ: &str = "apikey-read";
/// The built-in read-write role name.
pub const ROLE_APIKEY_WRITE: &str = "apikey-write";

/// The caps the `apikey-read` role bundles: read any store table, plus call any read-shaped tool
/// (`*.get` / `*.list`). Read-only is then enforced for free — `caps::check` denies any write/call
/// this set does not hold.
pub fn apikey_read_caps() -> Vec<String> {
    ["store:*:read", "mcp:*.get:call", "mcp:*.list:call"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// The caps the `apikey-write` role bundles: `apikey-read` plus store writes and the write-shaped
/// tool-call actions. Action-named (not `*.*`) so a data key can never reach `apikey.manage`.
pub fn apikey_write_caps() -> Vec<String> {
    let mut caps = apikey_read_caps();
    caps.extend(
        [
            "store:*:write",
            "mcp:*.write:call",
            "mcp:*.create:call",
            "mcp:*.update:call",
            "mcp:*.delete:call",
            "mcp:*.post:call",
        ]
        .iter()
        .map(|s| s.to_string()),
    );
    caps
}

/// The list-view badge for a key from its assigned role names: `read-only` if it carries
/// `apikey-read`, `read-write` if it carries `apikey-write`, else `custom` (a key granted only
/// fine-grained caps, or a non-built-in role). A key with both built-ins is `read-write`.
pub fn badge_for_roles(roles: &[String]) -> &'static str {
    let has = |name: &str| roles.iter().any(|r| r == name);
    if has(ROLE_APIKEY_WRITE) {
        "read-write"
    } else if has(ROLE_APIKEY_READ) {
        "read-only"
    } else {
        "custom"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_caps_are_a_superset_of_read_caps() {
        let read = apikey_read_caps();
        let write = apikey_write_caps();
        for c in &read {
            assert!(write.contains(c), "write role missing read cap {c}");
        }
    }

    #[test]
    fn no_built_in_cap_matches_the_management_resource() {
        // Neither role may carry a cap whose pattern would match `apikey.manage` — otherwise a data
        // key could administer keys. `*.write`/`*.get` etc. do not match `apikey.manage`.
        for cap in apikey_write_caps() {
            assert!(
                !cap.starts_with("mcp:*.*"),
                "write role must not carry a blanket *.* cap: {cap}"
            );
        }
        assert!(!apikey_read_caps().iter().any(|c| c.starts_with("mcp:*.*")));
    }

    #[test]
    fn badge_classifies_each_case() {
        assert_eq!(badge_for_roles(&[]), "custom");
        assert_eq!(
            badge_for_roles(&[ROLE_APIKEY_READ.to_string()]),
            "read-only"
        );
        assert_eq!(
            badge_for_roles(&[ROLE_APIKEY_WRITE.to_string()]),
            "read-write"
        );
        assert_eq!(
            badge_for_roles(&[ROLE_APIKEY_READ.to_string(), ROLE_APIKEY_WRITE.to_string()]),
            "read-write"
        );
        assert_eq!(badge_for_roles(&["custom-role".to_string()]), "custom");
    }
}
