//! One-shot boot migration (persona-session #5): copy a legacy `agent.config.active_persona` — #1's
//! retired workspace-global toggle — into the workspace-default prefs axis (`Prefs.agent_persona` on
//! `workspace_prefs:[ws]`), then clear the legacy field so a second boot is a no-op (idempotent).
//!
//! Posture:
//!   - Runs at boot right after `seed_personas` (both `node/src/main.rs` and the `test_gateway`
//!     harness), over every workspace the node knows: the `_lb_workspaces` directory ∪ the reactor
//!     directory (a workspace in either is a workspace whose config could carry the legacy field).
//!   - **Never overwrites** an already-set ws-default axis (an admin's newer prefs write wins; the
//!     legacy value is still cleared so it can't resurface).
//!   - A legacy id naming a since-deleted persona migrates anyway — the resolve fold's dangling-id
//!     path already warns + runs un-narrowed; no special case here.
//!   - Best-effort per workspace: one workspace's store error logs + skips, never fails the boot.
//!
//! This file is the ONLY reader of `active_persona` (the field is decode-only on `AgentConfig` and
//! dropped from `AGENT_CONFIG_COLUMNS`); it reads the raw column directly.

use lb_prefs::{get_workspace_prefs, set_workspace_prefs, Prefs};
use lb_store::{list as store_list, Store, StoreError};
use serde_json::Value;

use crate::agent::config::AGENT_CONFIG_TABLE;
use crate::directory::enabled_workspaces;
use crate::workspaces::{KIND as WS_KIND, TABLE as WS_TABLE, WORKSPACES_NS};

/// Migrate every known workspace's legacy `active_persona` into the ws-default prefs axis. Returns
/// the ids of the workspaces that actually carried (and migrated) a legacy value, for the boot log.
pub async fn migrate_active_persona(store: &Store) -> Result<Vec<String>, StoreError> {
    let mut migrated = Vec::new();
    for ws in known_workspaces(store).await? {
        match migrate_one(store, &ws).await {
            Ok(true) => migrated.push(ws),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("boot: active_persona migration skipped for ws {ws:?}: {e}");
            }
        }
    }
    Ok(migrated)
}

/// Migrate one workspace. `Ok(true)` when a legacy value was found and handled.
async fn migrate_one(store: &Store, ws: &str) -> Result<bool, StoreError> {
    let Some(legacy) = read_legacy_active_persona(store, ws).await? else {
        return Ok(false);
    };

    // Copy — unless an admin already set the ws-default axis (their newer write wins).
    let existing = get_workspace_prefs(store, ws)
        .await?
        .and_then(|p| p.agent_persona)
        .filter(|s| !s.is_empty());
    if existing.is_none() {
        let patch = Prefs {
            agent_persona: Some(legacy.clone()),
            ..Default::default()
        };
        set_workspace_prefs(store, ws, &patch).await?;
    }

    // Clear the legacy field either way (one-shot: a second boot finds nothing to copy).
    store
        .query_ws(
            ws,
            &format!("UPDATE type::thing('{AGENT_CONFIG_TABLE}', [$ws]) SET active_persona = NONE"),
            vec![("ws".into(), Value::String(ws.to_string()))],
        )
        .await?;
    tracing::info!("boot: migrated legacy active_persona {legacy:?} → ws-default prefs in {ws:?}");
    Ok(true)
}

/// Raw read of the legacy column — deliberately NOT `get_agent_config` (which no longer projects it).
async fn read_legacy_active_persona(store: &Store, ws: &str) -> Result<Option<String>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT active_persona FROM type::thing('{AGENT_CONFIG_TABLE}', [$ws])"),
            vec![("ws".into(), Value::String(ws.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(rows
        .into_iter()
        .next()
        .and_then(|row| row.get("active_persona").cloned())
        .and_then(|v| v.as_str().map(str::to_string))
        .filter(|s| !s.is_empty()))
}

/// Every workspace the node knows: the `_lb_workspaces` switcher directory (any status — an archived
/// workspace's config should still migrate) ∪ the reactor directory. Deduped, order irrelevant.
async fn known_workspaces(store: &Store) -> Result<Vec<String>, StoreError> {
    let mut out: Vec<String> = Vec::new();
    for row in store_list(store, WORKSPACES_NS, WS_TABLE, "kind", WS_KIND).await? {
        if let Some(ws) = row.get("ws").and_then(|v| v.as_str()) {
            if !out.iter().any(|w| w == ws) {
                out.push(ws.to_string());
            }
        }
    }
    for entry in enabled_workspaces(store).await? {
        if !out.iter().any(|w| w == &entry.ws) {
            out.push(entry.ws);
        }
    }
    Ok(out)
}
