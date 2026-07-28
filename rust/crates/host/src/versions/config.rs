//! `versions.config.get` / `versions.config.set` — the per-workspace ring-cap override
//! (`docs/scope/versions/entity-version-history-scope.md`, "The adjustable cap").
//!
//! `set` is **admin-only** (`ADMIN_ONLY_CAPS`): the cap decides how much of every member's work
//! history the workspace keeps, so lowering it destroys other people's recoverability. `get` is a
//! plain read a member holds — a client renders "20 kept" from it.
//!
//! `set` **merges, it does not replace** (the `series.retention.patch` house pattern): sending only
//! `{ per_kind: { flow: 50 } }` must not blank the workspace `cap`. A replacing write is available
//! by sending every field.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::{write, Store};
use serde::Serialize;
use serde_json::Value;

use super::cap::{
    clamp_cap, read_config, VersionsConfig, CONFIG_ID, CONFIG_TABLE, DEFAULT_VERSION_CAP,
    MAX_VERSION_CAP, MIN_VERSION_CAP,
};
use super::error::VersionsError;
use super::list::unknown_kind;
use super::plan::plan_for_kind;

/// The config as a client sees it — the stored override PLUS the node's own constants, so a UI can
/// render the bounds and the effective default without hardcoding them (and so a client cannot
/// drift from a node whose clamp changed).
#[derive(Debug, Serialize)]
pub struct ConfigView {
    /// The effective workspace-wide cap (the stored one, or the const).
    pub cap: usize,
    /// The per-kind overrides in force, clamped.
    pub per_kind: std::collections::BTreeMap<String, usize>,
    pub default_cap: usize,
    pub min_cap: usize,
    pub max_cap: usize,
}

fn view(cfg: &VersionsConfig) -> ConfigView {
    ConfigView {
        cap: clamp_cap(cfg.cap.unwrap_or(DEFAULT_VERSION_CAP)),
        per_kind: cfg
            .per_kind
            .iter()
            .map(|(k, v)| (k.clone(), clamp_cap(*v)))
            .collect(),
        default_cap: DEFAULT_VERSION_CAP,
        min_cap: MIN_VERSION_CAP,
        max_cap: MAX_VERSION_CAP,
    }
}

pub async fn versions_config_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<ConfigView, VersionsError> {
    authorize_tool(principal, ws, "versions.config.get").map_err(|_| VersionsError::Denied)?;
    Ok(view(&read_config(store, ws).await?))
}

/// Merge `cap` / `per_kind` into the workspace override. Every value is validated against the node
/// clamp **as a rejection**, not a silent clamp: an admin who types 500 must be told the ceiling is
/// 100, not silently given 100 and left believing they got 500.
pub async fn versions_config_set(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<ConfigView, VersionsError> {
    authorize_tool(principal, ws, "versions.config.set").map_err(|_| VersionsError::Denied)?;

    let mut cfg = read_config(store, ws).await?;

    if let Some(v) = input.get("cap") {
        if !v.is_null() {
            cfg.cap = Some(checked_cap(v, "cap")?);
        }
    }
    if let Some(pk) = input.get("per_kind") {
        if !pk.is_null() {
            let obj = pk
                .as_object()
                .ok_or_else(|| VersionsError::BadInput("`per_kind` must be an object".into()))?;
            for (kind, v) in obj {
                // An unknown kind is a REJECTION, not a stored no-op: a typo that silently persists
                // looks configured and does nothing, which is the worst of both.
                plan_for_kind(kind).ok_or_else(|| unknown_kind(kind))?;
                // An explicit null clears that kind's override (back to the workspace cap).
                if v.is_null() {
                    cfg.per_kind.remove(kind);
                } else {
                    cfg.per_kind
                        .insert(kind.clone(), checked_cap(v, &format!("per_kind.{kind}"))?);
                }
            }
        }
    }

    let value = serde_json::to_value(&cfg).map_err(|e| VersionsError::BadInput(e.to_string()))?;
    write(store, ws, CONFIG_TABLE, CONFIG_ID, &value).await?;
    Ok(view(&cfg))
}

/// Parse one cap value, rejecting anything outside the node's clamp range by NAME so the message
/// says which field was wrong.
fn checked_cap(v: &Value, field: &str) -> Result<usize, VersionsError> {
    let n = v
        .as_u64()
        .ok_or_else(|| VersionsError::BadInput(format!("`{field}` must be a positive integer")))?;
    let n = usize::try_from(n).unwrap_or(usize::MAX);
    if !(MIN_VERSION_CAP..=MAX_VERSION_CAP).contains(&n) {
        return Err(VersionsError::BadInput(format!(
            "`{field}` must be between {MIN_VERSION_CAP} and {MAX_VERSION_CAP} (got {n})"
        )));
    }
    Ok(n)
}
