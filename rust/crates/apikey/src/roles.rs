//! The two built-in roles an API key is minted under (api-keys scope), and the list-view badge
//! derived from a key's assigned roles. A key's permissions are just grants on `Subject::Key` —
//! read-only vs read-write and tool/page limits are *which caps the key resolves to*, enforced at
//! the existing chokepoint. These roles ship the common case in one click; finer custom caps are an
//! ordinary grant on `key:{id}`.
//!
//! The write role once granted **action-named** tool-call wildcards (`*.write`, `*.create`, …)
//! rather than a blanket `mcp:*.*:call`, reasoning that `*.*` would match the management resource
//! `apikey.manage` and let a data key mint/revoke other keys.
//!
//! **That reasoning was right about the mechanism and wrong about the blast radius** (fixed
//! 2026-07-16). Action-naming stops `apikey.manage`, but `*` still spans the `<tool>` half of every
//! `<tool>.<verb>` — so `mcp:*.list:call` reached the admin-only `teams.list` / `roles.list` /
//! `grants.list` / `invite.list`, and `mcp:*.delete:call` reached `workspace.delete`. A "read-only"
//! data key could enumerate the workspace's people, roles and grants; a read-write key could delete
//! the workspace. `apikey.manage` was never the only management resource — it was the one we thought
//! of, and a wildcard grants against the verbs that exist tomorrow, not the ones we enumerated today.
//!
//! So these bundles now NAME their data-plane verbs. The capability grammar is purely additive (there
//! is no deny form — see `lb-caps`), which means a bundle cannot subtract; the only way to bound a
//! bundle is to not over-grant it in the first place. `lb_host::authz` owns the admin-only list and
//! pins these bundles with `no_builtin_bundle_may_span_an_admin_only_cap` — the assertion cannot live
//! here, since `lb-apikey` sits below `lb-host` and must not depend on it.

/// The built-in read-only role name.
pub const ROLE_APIKEY_READ: &str = "apikey-read";
/// The built-in read-write role name.
pub const ROLE_APIKEY_WRITE: &str = "apikey-write";

/// The DATA-PLANE read verbs an `apikey-read` key bundles. Named concretely, never `mcp:*.get:call`
/// / `mcp:*.list:call` — those spanned the admin-only `teams.list` / `roles.list` / `grants.list`
/// (see the module note). `store:*:read` stays: it is a store-surface grant whose resource segment is
/// a table, so it cannot reach an `mcp:` management verb.
const APIKEY_READ_CAPS: &[&str] = &[
    "store:*:read",
    // series reads — the appliance polls its own data.
    "mcp:series.list:call",
    "mcp:series.read:call",
    "mcp:series.latest:call",
    "mcp:series.find:call",
    // dashboards / panels / reports the key renders (a LENS; sources re-check per call).
    "mcp:dashboard.get:call",
    "mcp:dashboard.list:call",
    "mcp:panel.get:call",
    "mcp:panel.list:call",
    "mcp:report.get:call",
    "mcp:report.list:call",
    // channels / media / insights the appliance reads.
    "mcp:channel.list:call",
    "mcp:media.list:call",
    "mcp:media.get:call",
    "mcp:insight.list:call",
    "mcp:insight.get:call",
    // saved queries + their compiled reads (query.run composes the target cap — no widening).
    "mcp:query.get:call",
    "mcp:query.list:call",
    "mcp:query.run:call",
];

/// The DATA-PLANE write verbs an `apikey-write` key adds. Named concretely, never
/// `mcp:*.write|create|update|delete|post:call` — those spanned the admin-only `invite.create` /
/// `nav.delete` and reached `workspace.delete` (see the module note).
const APIKEY_WRITE_CAPS: &[&str] = &[
    "store:*:write",
    // the appliance's reason to exist: push series data in. (`ingest.write` IS the series-append
    // path — there is no `series.append` verb; series writes ride ingest.)
    "mcp:ingest.write:call",
    // channel produce.
    "mcp:channel.post:call",
    "mcp:bus.publish:call",
    // media upload — a REST-surface cap (no MCP tool of that name), same as in the member bundle.
    "mcp:media.upload:call",
    // insight raise (an appliance reports conditions).
    "mcp:insight.raise:call",
];

/// The caps the `apikey-read` role bundles: read any store table, plus the named data-plane read
/// verbs. Read-only is then enforced for free — `caps::check` denies any write/call this set does
/// not hold.
pub fn apikey_read_caps() -> Vec<String> {
    APIKEY_READ_CAPS.iter().map(|s| s.to_string()).collect()
}

/// The caps the `apikey-write` role bundles: `apikey-read` plus the named data-plane write verbs.
/// Neither bundle reaches key administration or any other admin-only cap.
pub fn apikey_write_caps() -> Vec<String> {
    let mut caps = apikey_read_caps();
    caps.extend(APIKEY_WRITE_CAPS.iter().map(|s| s.to_string()));
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
