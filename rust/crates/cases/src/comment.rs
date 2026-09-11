//! `comment` — a human note on a case, stored as a `comment` case event (case-plane scope).
//!
//! **A case's notes are its history, not a second thread beside it.** Discussion, transitions,
//! requests and replies all read in ONE chronological list, which is what a person handing a job
//! over actually needs — a separate comment table would put half the story on another tab. So this
//! is [`crate::event_append`] with an [`EventKind::Comment`] and a `{ text }` payload, bounded by
//! the same 4 KB per-event refusal.
//!
//! `insight.comment` delegates here (resolved decision 5): the case owns the work, so the case owns
//! the conversation about it.

use lb_store::Store;

use crate::case_event::{EventKind, MAX_EVENT_DATA_BYTES};
use crate::error::CasesError;
use crate::event_append::append_event;
use crate::save::save;

/// Append `text` to case `id`'s history as `author`, returning the assigned event `seq`.
///
/// Refuses an empty note (indistinguishable from a mis-click, and the history's value is that every
/// row says something); the size cap is enforced by the event append itself, before any write.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.comment)
pub async fn comment(
    store: &Store,
    ws: &str,
    id: &str,
    text: &str,
    author: &str,
    ts: u64,
) -> Result<u64, CasesError> {
    if text.trim().is_empty() {
        return Err(CasesError::BadInput(
            "comment text is empty — a note must say something".into(),
        ));
    }
    if text.len() > MAX_EVENT_DATA_BYTES {
        return Err(CasesError::BadInput(format!(
            "comment {} bytes exceeds the {MAX_EVENT_DATA_BYTES}-byte cap — nothing was appended \
             and the existing history is untouched; a note is an operational comment, not a report",
            text.len()
        )));
    }
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    let seq = append_event(
        store,
        ws,
        id,
        EventKind::Comment,
        author,
        serde_json::json!({ "text": text }),
        ts,
    )
    .await?;
    // A note is activity: a case somebody is discussing must not read as untouched in the queue.
    save(store, ws, &mut case, ts).await?;
    Ok(seq)
}
