//! `authz.entity_scope { table, sources?, subject? }` — which ids of an entity table (e.g. `site`) a
//! principal may READ, answered by the SAME resolver lb enforces with ([`entity_scope`]: the union of
//! the `nav` and `grant` sources, workspace admins unrestricted).
//!
//! Why it exists next to `authz.scope_filter`: `scope_filter` answers from scoped GRANTS only, but a
//! product hands sites to people mainly through MENUS (`entity-scoped-data-scope.md`: "menus are
//! access"). An extension that filtered its own rows with `scope_filter` would therefore refuse a
//! viewer whose menu lb itself honours on every federated read, insight and case. Exposing the one
//! resolver means an extension and the core can never disagree about who reaches which entity — the
//! extension asks the wall instead of re-implementing it (entity-scoped-grants scope, rule 10: `table`
//! and the ids are opaque data here).
//!
//! **Whose reach?** The same contract as the other reach verbs (`scoped.rs`): no `subject` → the
//! caller's own scope, no extra cap; a present `subject` → that subject's scope, ONLY if the caller
//! holds `mcp:authz.delegate_reach:call`, else a hard deny (never a fallback to the caller's own).
//! Only `user:` subjects are accepted — a menu or a scoped grant is held by a user (through their
//! teams); a team or role has no reach of its own to report. Resolution reads only the caller's
//! workspace, so a `subject` cannot reach across the wall.
//!
//! Reply: `{ "filter": "all" }` or `{ "filter": { "ids": [...] } }` — the `scope_filter` shape, so an
//! extension handles both verbs with one decoder. An empty `ids` is "nothing", never "everything".

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use lb_store::Store;
use serde_json::{json, Value};

use super::entity_scope::{entity_scope, EntityScope, ENTITY_SCOPE_SOURCES};
use super::hold::holds_cap;
use super::resolve_live::resolve_caps_live;
use super::scoped::DELEGATE_REACH_CAP;
use super::tool::str_arg;

const USER_PREFIX: &str = "user:";

pub async fn authz_entity_scope(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, "authz.entity_scope")?;
    let table = str_arg(input, "table")?;
    let sources = sources_arg(input)?;
    let whose = whose(store, principal, ws, input).await?;
    Ok(
        match entity_scope(store, &whose, ws, table, &sources).await {
            EntityScope::All => json!({ "filter": "all" }),
            EntityScope::Ids(ids) => json!({ "filter": { "ids": ids } }),
        },
    )
}

/// The principal whose scope is resolved: the caller, or — with a `subject` and the delegation cap —
/// that user, rebuilt with their LIVE caps (the same rebuild [`entity_scope`] does for a derived
/// principal's owner), so their admin standing and team menus are today's, not a token's.
async fn whose(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Principal, ToolError> {
    let Some(subject) = input.get("subject").and_then(Value::as_str) else {
        return Ok(principal.clone());
    };
    if !holds_cap(principal, ws, DELEGATE_REACH_CAP) {
        return Err(ToolError::Denied);
    }
    let bare = subject
        .strip_prefix(USER_PREFIX)
        .filter(|b| !b.is_empty())
        .ok_or_else(|| {
            ToolError::BadInput(format!("subject must be a user:… id, got {subject:?}"))
        })?;
    let caps = resolve_caps_live(store, ws, bare)
        .await
        .map_err(|e| ToolError::Extension(e.to_string()))?;
    Ok(Principal::routed(subject, ws, caps))
}

/// The sources to union. Absent → every source lb knows (`nav` + `grant`). Present → each must be a
/// known source: an unknown name is refused rather than silently contributing nothing, so a typo can
/// never quietly shrink a scope to "nothing" and read as a denial of access.
fn sources_arg(input: &Value) -> Result<Vec<String>, ToolError> {
    let Some(raw) = input.get("sources").filter(|v| !v.is_null()) else {
        return Ok(ENTITY_SCOPE_SOURCES.iter().map(|s| s.to_string()).collect());
    };
    let list = raw
        .as_array()
        .ok_or_else(|| ToolError::BadInput("sources must be a list".into()))?;
    if list.is_empty() {
        return Err(ToolError::BadInput(
            "sources must name at least one source".into(),
        ));
    }
    list.iter()
        .map(|v| match v.as_str() {
            Some(s) if ENTITY_SCOPE_SOURCES.contains(&s) => Ok(s.to_string()),
            _ => Err(ToolError::BadInput(format!(
                "unknown source {v}; known: {}",
                ENTITY_SCOPE_SOURCES.join(", ")
            ))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_sources_mean_every_known_source() {
        assert_eq!(sources_arg(&json!({})).unwrap(), vec!["nav", "grant"]);
        assert_eq!(
            sources_arg(&json!({"sources": null})).unwrap(),
            vec!["nav", "grant"]
        );
    }

    #[test]
    fn an_unknown_or_empty_source_list_is_refused_not_ignored() {
        assert!(sources_arg(&json!({"sources": ["nav", "tags"]})).is_err());
        assert!(sources_arg(&json!({"sources": []})).is_err());
        assert!(sources_arg(&json!({"sources": "nav"})).is_err());
        assert_eq!(
            sources_arg(&json!({"sources": ["nav"]})).unwrap(),
            vec!["nav"]
        );
    }
}
