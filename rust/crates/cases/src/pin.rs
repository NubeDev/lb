//! `pin` — pin a case for yourself, or take your pin off (Insights review, 2026-10).
//!
//! **A pin is a person's own.** It is a bookmark, not a team flag: the case keeps the set of
//! subjects who pinned it (`pinned_by`), and the queue's `pinned` axis reads only the caller's
//! ([`crate::ListFilter::pinned_for`]). Because it is nobody else's business it is not work on the
//! case either: it appends no history event and does not bump `last_activity_ts`, so pinning never
//! makes a case look touched in the queue or to self-clear.
//!
//! Idempotent: pinning what you already pinned (or unpinning what you did not) writes nothing.
//!
//! One responsibility: add or remove one subject's pin on a case.

use lb_store::{write, Store};

use crate::case::{Case, TABLE};
use crate::error::CasesError;

/// Pin case `id` for `subject` (`pinned: true`), or remove their pin.
// SCOPE: docs/scope/insights/case-plane-scope.md §"Verbs" (case.pin)
pub async fn pin(
    store: &Store,
    ws: &str,
    id: &str,
    pinned: bool,
    subject: &str,
) -> Result<Case, CasesError> {
    let Some(mut case) = crate::get::get(store, ws, id).await? else {
        return Err(CasesError::BadInput(format!("no such case: {id}")));
    };
    let has = case.pinned_by.iter().any(|s| s == subject);
    if has == pinned {
        return Ok(case);
    }
    if pinned {
        case.pinned_by.push(subject.to_string());
    } else {
        case.pinned_by.retain(|s| s != subject);
    }
    // Not `save`: that stamps `last_activity_ts`, and a bookmark is not activity (module note).
    let value = serde_json::to_value(&case).map_err(CasesError::decode)?;
    write(store, ws, TABLE, &case.id, &value).await?;
    Ok(case)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::case::Grouping;
    use crate::open::{open, OpenInput};

    async fn seed(store: &Store) -> Case {
        let input = OpenInput {
            title: "a job".into(),
            grouping: Grouping::Single,
            primary_insight: "i-1".into(),
            severity: "warning".into(),
            ..Default::default()
        };
        open(store, "nube", input, "user:test", 1).await.unwrap()
    }

    /// Each person's pin is their own: two people pin, one unpins, the other's stays. A repeat
    /// writes nothing, no pin touches the history, and none makes the case look worked.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_pin_is_each_persons_own() {
        let store = Store::memory().await.unwrap();
        let opened = seed(&store).await;
        let id = opened.id.clone();
        let history = crate::events::events(&store, "nube", &id, 50, None)
            .await
            .unwrap()
            .items
            .len();

        pin(&store, "nube", &id, true, "user:priya").await.unwrap();
        pin(&store, "nube", &id, true, "user:sam").await.unwrap();
        let again = pin(&store, "nube", &id, true, "user:priya").await.unwrap();
        assert_eq!(again.pinned_by, vec!["user:priya", "user:sam"]);

        let after = pin(&store, "nube", &id, false, "user:sam").await.unwrap();
        assert_eq!(after.pinned_by, vec!["user:priya"]);
        assert_eq!(after.last_activity_ts, opened.last_activity_ts);
        let page = crate::events::events(&store, "nube", &id, 50, None)
            .await
            .unwrap();
        assert_eq!(
            page.items.len(),
            history,
            "a pin is not in the case's history"
        );
    }
}
