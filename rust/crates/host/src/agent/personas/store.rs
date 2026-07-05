//! The `persona` table + its raw store verbs (persona-model scope). Two tiers share one shape and one
//! table name; only the **namespace** differs — the `agent_definition` store pattern, cloned:
//!   - built-ins live once, node-level, in the reserved [`PERSONA_NS`] (`_lb_personas`), the
//!     `_lb_agents` / `_lb_skills` precedent — the boot seeder is the ONLY writer of that namespace;
//!   - custom entries live per-workspace, in the workspace namespace (the hard wall).
//!
//! Every row carries a `kind: "persona"` discriminator so the generic `list` verb selects personas by
//! an equality filter. The store is schemaless (`lb_store::write` UPSERTs `CONTENT` with a managed
//! `rev`) — no DDL. Composite id `[slug]` → an offline custom write UPSERTs idempotently on replay
//! (LWW). Raw verbs: no authorization here — they run *after* the host capability gate.

use lb_store::{delete as store_delete, list as store_list, read, write, Store, StoreError};
use serde_json::{json, Value};

use super::model::{Persona, PERSONA_KIND};

/// The reserved node-level namespace the built-in personas are seeded into. Leading underscore marks
/// it system-internal (mirrors `_lb_agents` / `_lb_skills`); an operator must never name a real
/// workspace this.
pub const PERSONA_NS: &str = "_lb_personas";

/// The one table both tiers share.
pub const PERSONA_TABLE: &str = "persona";

/// Read one persona by id from `ns` (a workspace namespace for custom, [`PERSONA_NS`] for built-ins).
/// `builtin` is stamped on the way out from the caller's knowledge of the namespace, so the record
/// itself never has to be trusted for its tier. `None` if absent in *this* namespace.
pub async fn get_persona(
    store: &Store,
    ns: &str,
    id: &str,
    builtin: bool,
) -> Result<Option<Persona>, StoreError> {
    match read(store, ns, PERSONA_TABLE, id).await? {
        Some(value) => {
            let mut persona: Persona =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            persona.builtin = builtin;
            Ok(Some(persona))
        }
        None => Ok(None),
    }
}

/// List every persona in `ns`, tier-stamped `builtin`. Filters on the `kind` discriminator so an
/// unrelated row in the same namespace is never returned. Sorted by id for a stable catalog order.
pub async fn list_personas(
    store: &Store,
    ns: &str,
    builtin: bool,
) -> Result<Vec<Persona>, StoreError> {
    let rows = store_list(store, ns, PERSONA_TABLE, "kind", PERSONA_KIND).await?;
    let mut personas: Vec<Persona> = rows
        .into_iter()
        .map(|v| {
            serde_json::from_value::<Persona>(v).map_err(|e| StoreError::Decode(e.to_string()))
        })
        .collect::<Result<_, _>>()?;
    for p in &mut personas {
        p.builtin = builtin;
    }
    personas.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(personas)
}

/// Upsert `persona` into `ns` (LWW on the composite `[id]`). The stored value carries the persona's
/// fields plus the `kind` discriminator; `builtin` is NOT stored (it is derived from the namespace on
/// read — never a spoofable flag). The caller has already gated caps + the reserved-tier check.
pub async fn upsert_persona(store: &Store, ns: &str, persona: &Persona) -> Result<(), StoreError> {
    let mut value = serde_json::to_value(persona).map_err(|e| StoreError::Decode(e.to_string()))?;
    if let Value::Object(map) = &mut value {
        map.insert("kind".into(), json!(PERSONA_KIND));
        // `builtin` is a read-time derivation from the namespace, never a stored (spoofable) flag.
        map.remove("builtin");
    }
    write(store, ns, PERSONA_TABLE, &persona.id, &value).await
}

/// Delete persona `id` from `ns`. Custom-only in practice (the verb layer rejects a built-in id before
/// reaching here); a no-op if absent.
pub async fn delete_persona(store: &Store, ns: &str, id: &str) -> Result<(), StoreError> {
    store_delete(store, ns, PERSONA_TABLE, id).await
}
