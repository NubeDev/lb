//! The `agent_definition` table + its raw store verbs (agent-catalog scope). Two tiers share one
//! shape and one table name; only the **namespace** differs:
//!   - built-ins live once, node-level, in the reserved [`AGENT_DEFS_NS`] (`_lb_agents`), the
//!     `_lb_skills` / `_lb_identity` precedent — the boot seeder is the ONLY writer of that namespace;
//!   - custom entries live per-workspace, in the workspace namespace (the hard wall).
//!
//! Every row carries a `kind: "agent_definition"` discriminator so the generic `list` verb selects
//! definitions by an equality filter without ordering assumptions. Composite id `[slug]` → an offline
//! custom write UPSERTs idempotently on replay (LWW), exactly like `workspace_agent_config`. Raw
//! verbs: no authorization here — they run *after* the host capability gate.

use lb_store::{delete as store_delete, list as store_list, read, write, Store, StoreError};
use serde_json::{json, Value};

use super::model::{AgentDefinition, DEFINITION_KIND};

/// The reserved node-level namespace the built-in definitions are seeded into. Leading underscore
/// marks it system-internal (mirrors `_lb_skills` / `_lb_identity`); an operator must never name a
/// real workspace this.
pub const AGENT_DEFS_NS: &str = "_lb_agents";

/// The one table both tiers share.
pub const AGENT_DEFS_TABLE: &str = "agent_definition";

/// Read one definition by id from `ns` (a workspace namespace for custom, [`AGENT_DEFS_NS`] for
/// built-ins). `builtin` is stamped on the way out from the caller's knowledge of the namespace, so
/// the record itself never has to be trusted for its tier. `None` if absent in *this* namespace.
pub async fn get_definition(
    store: &Store,
    ns: &str,
    id: &str,
    builtin: bool,
) -> Result<Option<AgentDefinition>, StoreError> {
    match read(store, ns, AGENT_DEFS_TABLE, id).await? {
        Some(value) => {
            let mut def: AgentDefinition =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            def.builtin = builtin;
            Ok(Some(def))
        }
        None => Ok(None),
    }
}

/// List every definition in `ns`, tier-stamped `builtin`. Filters on the `kind` discriminator so an
/// unrelated row in the same namespace is never returned. Sorted by id for a stable catalog order.
pub async fn list_definitions(
    store: &Store,
    ns: &str,
    builtin: bool,
) -> Result<Vec<AgentDefinition>, StoreError> {
    let rows = store_list(store, ns, AGENT_DEFS_TABLE, "kind", DEFINITION_KIND).await?;
    let mut defs: Vec<AgentDefinition> = rows
        .into_iter()
        .map(|v| {
            serde_json::from_value::<AgentDefinition>(v)
                .map_err(|e| StoreError::Decode(e.to_string()))
        })
        .collect::<Result<_, _>>()?;
    for d in &mut defs {
        d.builtin = builtin;
    }
    defs.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(defs)
}

/// Upsert `def` into `ns` (LWW on the composite `[id]`). The stored value carries the definition's
/// fields plus the `kind` discriminator and a denormalized `id` for query convenience; `builtin` is
/// NOT stored (it is derived from the namespace on read). The caller has already gated caps + the
/// reserved-tier check.
pub async fn upsert_definition(
    store: &Store,
    ns: &str,
    def: &AgentDefinition,
) -> Result<(), StoreError> {
    let mut value = serde_json::to_value(def).map_err(|e| StoreError::Decode(e.to_string()))?;
    if let Value::Object(map) = &mut value {
        map.insert("kind".into(), json!(DEFINITION_KIND));
        // `builtin` is a read-time derivation from the namespace, never a stored (spoofable) flag.
        map.remove("builtin");
    }
    write(store, ns, AGENT_DEFS_TABLE, &def.id, &value).await
}

/// Delete definition `id` from `ns`. Custom-only in practice (the verb layer rejects a built-in id
/// before reaching here); a no-op if absent.
pub async fn delete_definition(store: &Store, ns: &str, id: &str) -> Result<(), StoreError> {
    store_delete(store, ns, AGENT_DEFS_TABLE, id).await
}
