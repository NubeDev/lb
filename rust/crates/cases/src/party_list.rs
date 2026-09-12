//! `party_list` / `party_get` — read the workspace's party roster (case-plane scope, wave 2).
//!
//! Two reads of one table, kept together because the second is the first narrowed to one id and a
//! separate file for a three-line `read` would be filing, not layering.
//!
//! The list is ordered by `name` (case-insensitive), then `id` — a roster is read by a human
//! looking for a company, so it sorts the way a contact list does. Optional filters by `kind` and by
//! `site` are ANDed; both values are opaque workspace strings this crate never interprets (rule 10).
//!
//! **No scorecard columns.** "Answered 8 of 11, median 4 h" is a question about `case_request` rows
//! and is derived by query at read time by whoever asks — see `party.rs`'s module note.

use lb_store::{read, scan_all, Store};

use crate::error::CasesError;
use crate::party::{Party, PartyKind, TABLE};

/// Read every party in `ws`, optionally narrowed to one `kind` and/or one `site`.
///
/// **Retired parties are excluded unless `include_disabled`.** The default serves the caller that
/// is choosing a recipient — a picker offering a company you have stopped using is offering a
/// mistake — while the settings surface passes `true`, because an admin cannot re-enable a row they
/// cannot see. [`party_get`] is deliberately NOT filtered: a request already sent to a retired
/// party must still resolve its name for the history and the nudge ladder.
///
/// A row that fails to decode is **skipped**, not fatal — one malformed row must not blank the
/// roster page and with it the admin's only way to fix it (the `policy_list` precedent).
pub async fn party_list(
    store: &Store,
    ws: &str,
    kind: Option<PartyKind>,
    site: Option<&str>,
    include_disabled: bool,
) -> Result<Vec<Party>, CasesError> {
    let rows = scan_all(store, ws, TABLE).await?;
    let mut parties: Vec<Party> = rows
        .into_iter()
        .filter_map(|row| unwrap_party(row.data))
        .filter(|p| include_disabled || p.active)
        .filter(|p| kind.is_none_or(|k| p.kind == k))
        .filter(|p| site.is_none_or(|s| p.sites.iter().any(|own| own == s)))
        .collect();
    parties.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(parties)
}

/// Read one party by id, or `None` if absent in this workspace.
pub async fn party_get(store: &Store, ws: &str, id: &str) -> Result<Option<Party>, CasesError> {
    let Some(value) = read(store, ws, TABLE, id).await? else {
        return Ok(None);
    };
    Ok(Some(
        serde_json::from_value(value).map_err(CasesError::decode)?,
    ))
}

/// Unwrap the `{ data, rev }` write envelope `scan` returns, then decode. `read`/`list` hand back
/// the inner value already unwrapped; `scan` does not (`store-data-envelope`).
fn unwrap_party(row: serde_json::Value) -> Option<Party> {
    let inner = match row {
        serde_json::Value::Object(mut obj) => {
            obj.remove("data").unwrap_or(serde_json::Value::Object(obj))
        }
        other => other,
    };
    serde_json::from_value(inner).ok()
}
