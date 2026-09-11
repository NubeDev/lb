//! `member_add` — cite one insight from one case, and the place the **exclusivity invariant** is
//! enforced (case-plane scope).
//!
//! > Every open insight is in exactly ONE open case.
//!
//! So an add that would put an insight into a second open case is **refused**. That refusal is the
//! invariant: without it "how many open jobs do we have" double-counts, and two technicians work
//! the same fault from two queues. The verb that legitimately moves a detection between cases is
//! [`crate::merge`] (or [`crate::split`]) — both remove before they add, so neither trips this.
//!
//! Idempotent: re-adding an insight to the case that already holds it writes nothing and keeps the
//! stored `human_placed` — a reactor re-running its grouping must never downgrade a person's
//! judgement to a machine's.

use lb_store::{write, Store};

use crate::case_member::{member_id, CaseMember, MemberRole, TABLE};
use crate::error::CasesError;
use crate::find_by_insight::find_open_case_for_insight;

/// Add `insight_id` to `case_id` in workspace `ws`.
///
/// Refuses when the case does not exist, and when the insight is already held by a DIFFERENT open
/// case. Returns the membership row as stored.
// Eight arguments, deliberately positional: every one is a distinct fact about the write, and a
// parameter struct here would add a type whose only job is to be destructured one line later. The
// host layer is the only caller.
#[allow(clippy::too_many_arguments)]
pub async fn member_add(
    store: &Store,
    ws: &str,
    case_id: &str,
    insight_id: &str,
    role: MemberRole,
    added_by: &str,
    human_placed: bool,
    ts: u64,
) -> Result<CaseMember, CasesError> {
    if crate::get::get(store, ws, case_id).await?.is_none() {
        return Err(CasesError::BadInput(format!("no such case: {case_id}")));
    }

    if let Some((existing_case, existing)) =
        find_open_case_for_insight(store, ws, insight_id).await?
    {
        if existing_case.id != case_id {
            return Err(CasesError::BadInput(format!(
                "insight {insight_id} is already a member of open case {} — every open insight is \
                 in exactly one open case; merge the cases or resolve one first",
                existing_case.id
            )));
        }
        // Already here. Write nothing, and never downgrade a human placement to a machine one.
        if existing.role == role && (existing.human_placed || !human_placed) {
            return Ok(existing);
        }
    }

    // Preserve a stored human placement across a role change — the flag is a fact about WHO put the
    // insight here, and a reactor re-labelling the role does not un-make that.
    let stored_human = crate::find_by_insight::memberships_of_insight(store, ws, insight_id)
        .await?
        .into_iter()
        .find(|m| m.case_id == case_id)
        .map(|m| m.human_placed)
        .unwrap_or(false);

    let member = CaseMember {
        case_id: case_id.to_string(),
        insight_id: insight_id.to_string(),
        role,
        added_by: added_by.to_string(),
        human_placed: human_placed || stored_human,
        ts,
    };
    let value = serde_json::to_value(&member).map_err(CasesError::decode)?;
    write(store, ws, TABLE, &member_id(case_id, insight_id), &value).await?;
    Ok(member)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::case::{Grouping, Resolution, Workflow};
    use crate::open::{open, OpenInput};
    use lb_store::Store;

    fn input(title: &str, primary: &str) -> OpenInput {
        OpenInput {
            title: title.into(),
            grouping: Grouping::Single,
            primary_insight: primary.into(),
            severity: "warning".into(),
            ..Default::default()
        }
    }

    /// The invariant, at crate altitude and against a REAL store: a second OPEN case cannot cite an
    /// insight the first one holds, and the refusal names the case that already has it.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_second_open_case_cannot_cite_an_insight_the_first_one_holds() {
        let store = Store::memory().await.unwrap();
        let first = open(&store, "nube", input("first", "i-1"), "user:test", 1)
            .await
            .unwrap();
        let second = open(&store, "nube", input("second", "i-2"), "user:test", 2)
            .await
            .unwrap();

        let err = member_add(
            &store,
            "nube",
            &second.id,
            "i-1",
            MemberRole::Explained,
            "system:case-group",
            false,
            3,
        )
        .await
        .unwrap_err();
        let CasesError::BadInput(message) = err else {
            panic!("expected BadInput");
        };
        assert!(message.contains(&first.id), "names the holder: {message}");

        // Closing the first case frees the insight — an insight may sit in MANY closed cases (one
        // detection key, three jobs over three years), which is why the filter is on `closed`.
        crate::workflow::workflow(
            &store,
            "nube",
            &first.id,
            Workflow::Resolved,
            Some(Resolution::Fixed),
            None,
            "user:test",
            4,
        )
        .await
        .unwrap();
        member_add(
            &store,
            "nube",
            &second.id,
            "i-1",
            MemberRole::Explained,
            "system:case-group",
            false,
            5,
        )
        .await
        .expect("a CLOSED case does not hold the insight");
    }

    /// A reactor re-running its grouping must never downgrade a person's judgement to a machine's:
    /// re-adding a `human_placed` member with `human_placed: false` keeps the flag.
    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn a_re_add_never_downgrades_a_human_placement() {
        let store = Store::memory().await.unwrap();
        let case = open(&store, "nube", input("c", "i-1"), "user:test", 1)
            .await
            .unwrap();
        member_add(
            &store,
            "nube",
            &case.id,
            "i-2",
            MemberRole::Explained,
            "user:test",
            true,
            2,
        )
        .await
        .unwrap();

        let again = member_add(
            &store,
            "nube",
            &case.id,
            "i-2",
            MemberRole::Storm,
            "system:case-group",
            false,
            3,
        )
        .await
        .unwrap();
        assert!(
            again.human_placed,
            "a machine re-add must not clear the human flag"
        );
    }
}
