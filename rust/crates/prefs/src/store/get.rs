//! Read a user's stored `user_prefs:[ws,user]` record — the raw nullable [`Prefs`], or `None` if the
//! user never set any axis. Namespace-scoped (the hard wall). Raw verb: runs *after* the host's
//! capability gate; this file does no authorization.

use lb_store::{Store, StoreError};
use serde_json::Value;

use crate::prefs::Prefs;

use super::schema::{PREFS_COLUMNS, USER_PREFS_TABLE};

/// Load `user`'s preference record in `ws`. `Ok(None)` when no record exists (every axis would
/// inherit). Decodes the stored row into the nullable [`Prefs`] shape.
pub async fn get_user_prefs(
    store: &Store,
    ws: &str,
    user: &str,
) -> Result<Option<Prefs>, StoreError> {
    let mut resp = store
        .query_ws(
            ws,
            &format!("SELECT {PREFS_COLUMNS} FROM type::thing('{USER_PREFS_TABLE}', [$ws, $user])"),
            vec![
                ("ws".into(), Value::String(ws.to_string())),
                ("user".into(), Value::String(user.to_string())),
            ],
        )
        .await?;
    let rows: Vec<Value> = resp
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    decode_first(rows)
}

/// Decode the first row (if any) into a [`Prefs`], ignoring the bookkeeping `id`/`ws`/`user`
/// columns serde does not know about.
pub(super) fn decode_first(rows: Vec<Value>) -> Result<Option<Prefs>, StoreError> {
    match rows.into_iter().next() {
        None => Ok(None),
        Some(row) => {
            let prefs: Prefs =
                serde_json::from_value(row).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(prefs))
        }
    }
}
