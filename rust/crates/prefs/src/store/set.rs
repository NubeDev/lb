//! Upsert a user's `user_prefs:[ws,user]` record from a patch (prefs scope `prefs.set`). The patch
//! is a [`Prefs`] where a present axis sets that field and an absent axis leaves the stored value
//! untouched (MERGE semantics) — so a user can change one axis without clearing the rest.
//!
//! The composite id is deterministic, so an offline edit replays idempotently as the same UPSERT
//! (last-writer-wins on a contested field) — no duplicate record. Namespace-scoped. Raw verb: the
//! host's capability gate (`prefs.set` = write OWN) runs first; `user` here is already the caller's
//! own sub (the host forces it, never caller-supplied for another user).

use lb_store::{Store, StoreError};
use serde_json::{json, Value};

use crate::prefs::Prefs;

use super::schema::{define_prefs_schema, USER_PREFS_TABLE};

/// Apply `patch` to `user`'s record in `ws`, creating it if absent. Present fields overwrite; the
/// `id`/`ws`/`user` bookkeeping columns are always set so the row is self-describing.
pub async fn set_user_prefs(
    store: &Store,
    ws: &str,
    user: &str,
    patch: &Prefs,
) -> Result<(), StoreError> {
    define_prefs_schema(store, ws).await?;
    // MERGE the present fields onto the (possibly existing) record. UPSERT ... MERGE keeps untouched
    // fields; absent axes in `patch` (skipped by serde's skip_serializing_if) are simply not in the
    // merge object, so they stay as stored.
    let mut merge = patch_object(patch)?;
    merge.insert("ws".into(), Value::String(ws.to_string()));
    merge.insert("user".into(), Value::String(user.to_string()));

    store
        .query_ws(
            ws,
            &format!("UPSERT type::thing('{USER_PREFS_TABLE}', [$ws, $user]) MERGE $patch"),
            vec![
                ("ws".into(), Value::String(ws.to_string())),
                ("user".into(), Value::String(user.to_string())),
                ("patch".into(), Value::Object(merge)),
            ],
        )
        .await?;
    Ok(())
}

/// Serialize `patch` to a JSON object of only its present axes (the `skip_serializing_if` on each
/// `Option`/empty-map field does the filtering).
pub(super) fn patch_object(patch: &Prefs) -> Result<serde_json::Map<String, Value>, StoreError> {
    match serde_json::to_value(patch).map_err(|e| StoreError::Decode(e.to_string()))? {
        Value::Object(map) => Ok(map),
        other => Ok(json!({ "_": other })
            .as_object()
            .cloned()
            .unwrap_or_default()),
    }
}
