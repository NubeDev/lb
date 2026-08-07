//! Entity-scoped **reach filtering** for a `viz.query` target (entity-scoped option sources — the
//! Forms-10x non-blocking ask; sits alongside the pack-store-datasource entity-grant read path).
//!
//! A picker/option-source target names a store-backed entity by carrying an OPTIONAL `entity` hint —
//! `{ table, cap, pk? }`, all opaque data (rule 10 — the core never special-cases a named entity).
//! When present, the resolver applies the SAME [`scope_filter`](lb_authz::scope_filter) the entity's
//! `.list` verb applies: it asks the caps wall "which rows in `table` may THIS caller reach under
//! `cap`?" and keeps only those rows. So a tech whose entity-grant reaches `ems_site:[north]` sees
//! only `north` in the picker — the exact reach `ems.site.list` enforces — even when the target is a
//! raw `store.query` on the entity table rather than the typed `.list` verb.
//!
//! **It never widens.** The hint is a tightening lens, additive and opt-in:
//!   - no hint ⇒ this module is never called; today's path byte-for-byte.
//!   - `ScopeFilter::All` (the caller has full reach — an admin, or an unscoped grant) ⇒ rows pass
//!     through UNCHANGED (no filtering, no cost beyond the one resolve).
//!   - `ScopeFilter::Ids(reachable)` ⇒ keep only rows whose `pk` value ∈ `reachable`.
//!
//! **Clean degradation on a non-entity target.** If the result set does not carry the `pk` column at
//! all (the hint was attached to a target that isn't that entity table — a series read, a mis-hinted
//! source), the rows pass through unchanged: an entity hint on a non-entity result is inert, never an
//! error and never a silent blank. A store-read error while resolving reach fails CLOSED (empty) — a
//! reach filter that cannot determine reach must not leak rows.

use lb_auth::Principal;
use lb_authz::{scope_filter_with, ScopeFilter};
use lb_store::Store;
use serde_json::Value;
use std::collections::BTreeSet;

use crate::authz::LiveBuiltinRoleCaps;

/// The entity a picker target names so `viz.query` honors its entity-grant reach. All fields are
/// opaque to the core: `table` and `cap` are the extension's, `pk` is the record-id column.
///
/// - `table` — the store table the entity's rows live in (the pack binding's `Entity.table`).
/// - `cap` — the cap the entity-grant is scoped under (the entity's `.list` verb cap, e.g.
///   `mcp:ems.site.list:call`). This is the key [`scope_filter`] resolves the reach for.
/// - `pk` — the record-id column of `table` (the pack binding's `Entity.pk`); the value compared
///   against the reachable id set. Defaults to `"id"`.
#[derive(Debug, Clone)]
pub struct EntityReach {
    pub table: String,
    pub cap: String,
    pub pk: Option<String>,
}

impl EntityReach {
    /// Parse the OPTIONAL `entity` hint off a source/target spec. Requires a non-empty `table` AND
    /// `cap`; anything else (absent, not an object, missing either key) ⇒ `None` — the hint is simply
    /// not applied (a malformed hint degrades to "no reach filter", never an error). `pk` is optional.
    pub fn from_value(v: Option<&Value>) -> Option<Self> {
        let obj = v?.as_object()?;
        let table = obj.get("table").and_then(Value::as_str).unwrap_or("");
        let cap = obj.get("cap").and_then(Value::as_str).unwrap_or("");
        if table.is_empty() || cap.is_empty() {
            return None;
        }
        Some(Self {
            table: table.to_string(),
            cap: cap.to_string(),
            pk: obj
                .get("pk")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        })
    }

    /// The record-id column — the author's `pk`, or `"id"`.
    fn pk(&self) -> &str {
        self.pk.as_deref().unwrap_or("id")
    }
}

/// Apply the entity-grant reach filter to `rows` — the SAME `scope_filter` the entity's `.list` verb
/// applies, at the viz layer. `caller`'s reach is resolved from the REAL store (latest grants, the
/// live built-in roles), never from the token — so a just-revoked grant bites here too.
///
/// Returns the reachable subset (or all rows when reach is `All` / the hint doesn't match this
/// result). See the module doc for the full degradation contract.
pub async fn apply_entity_reach(
    store: &Store,
    ws: &str,
    caller: &Principal,
    entity: &EntityReach,
    rows: Vec<Value>,
) -> Vec<Value> {
    // The bare subject the grants are stored under (`Subject::User("test")`, not `"user:test"`), matching
    // the host `authz.scope_filter` bridge. A non-user subject has no scoped grants ⇒ empty reach.
    let user = caller.sub().strip_prefix("user:").unwrap_or(caller.sub());

    let reachable = match scope_filter_with(
        store,
        ws,
        user,
        &entity.cap,
        &entity.table,
        &LiveBuiltinRoleCaps,
    )
    .await
    {
        // Full reach — the caller (admin / unscoped grant) sees every row. Zero filtering.
        Ok(ScopeFilter::All) => return rows,
        Ok(ScopeFilter::Ids(ids)) => ids,
        // Could not determine reach (a store read error) — fail CLOSED. A reach filter that cannot
        // read the grants must not pass rows through; an honest empty beats a silent leak.
        Err(_) => return Vec::new(),
    };

    // Clean degradation: if the result carries no `pk` column at all, the hint was attached to a
    // target that isn't this entity table (a series read, a mis-hint). Leave the rows untouched —
    // an entity hint on a non-entity result is inert, never a silent blank.
    let pk = entity.pk();
    if !rows.iter().any(|r| r.get(pk).is_some()) {
        return rows;
    }

    let set: BTreeSet<&str> = reachable.iter().map(String::as_str).collect();
    rows.into_iter()
        .filter(|r| {
            row_id(r.get(pk))
                .map(|id| set.contains(id.as_str()))
                .unwrap_or(false)
        })
        .collect()
}

/// The record-id of a `pk` cell as a string, for membership against the reachable set. A JSON string
/// is taken verbatim; a number is stringified (a numeric pk). Anything else (object/array/null/absent)
/// ⇒ `None` — an un-id-able row is not reachable (dropped once the `pk` column is known to exist).
fn row_id(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}
