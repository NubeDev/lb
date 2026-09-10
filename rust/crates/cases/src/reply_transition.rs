//! **What a reply does to the case** — the six-row transition table, in one pure function
//! (case-plane scope, wave 2; product scope §6 and its example flow).
//!
//! | reply | workflow | `waiting_on` | why |
//! |---|---|---|---|
//! | `accept`    | `actioned`      | `contractor` | they said yes; the ball is theirs |
//! | `quote`     | `waiting_on_po` | `client`     | the job is now blocked on money, not effort (the example flow's step 5→6: the FM raises the PO next) |
//! | `eta`       | `actioned`      | `contractor` | a date is a commitment to attend |
//! | `done`      | `actioned`      | `internal`   | **not** `resolved` — see below |
//! | `need_info` | `to_action`     | `internal`   | the ball comes back to us |
//! | `decline`   | `to_action`     | `internal`   | we have to find someone else |
//!
//! **Why `done` does not resolve the case.** Closing needs a `Resolution` — `fixed` vs
//! `self_cleared` vs `false_positive` — and that is a judgment about *our* detection, made by
//! someone accountable for it. A contractor saying "finished" is evidence, not a verdict, and a
//! table that let a token principal write `resolved` would hand the outside world the one
//! transition the whole scorecard is computed from. So `done` moves the work to `actioned` and puts
//! the ball back inside, where a person closes it with a reason.
//!
//! A pure function over two enums: no store, no clock, no authorization. It is the one statement of
//! this mapping, so the reply verb and any UI that previews "what will this do" cannot disagree.

use crate::case::{WaitingOn, Workflow};
use crate::case_request::ReplyKind;

/// The `(workflow, waiting_on)` a reply of `kind` moves the case to.
pub fn transition_for(kind: ReplyKind) -> (Workflow, WaitingOn) {
    match kind {
        ReplyKind::Accept => (Workflow::Actioned, WaitingOn::Contractor),
        ReplyKind::Quote => (Workflow::WaitingOnPo, WaitingOn::Client),
        ReplyKind::Eta => (Workflow::Actioned, WaitingOn::Contractor),
        ReplyKind::Done => (Workflow::Actioned, WaitingOn::Internal),
        ReplyKind::NeedInfo => (Workflow::ToAction, WaitingOn::Internal),
        ReplyKind::Decline => (Workflow::ToAction, WaitingOn::Internal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_reply_kind_can_resolve_a_case() {
        for kind in [
            ReplyKind::Accept,
            ReplyKind::Quote,
            ReplyKind::Eta,
            ReplyKind::Done,
            ReplyKind::NeedInfo,
            ReplyKind::Decline,
        ] {
            let (workflow, _) = transition_for(kind);
            assert!(
                !workflow.is_terminal(),
                "{kind:?} must not close the case — closing requires a resolution a party cannot give"
            );
        }
    }

    #[test]
    fn a_quote_blocks_on_money_and_an_acceptance_blocks_on_the_contractor() {
        assert_eq!(
            transition_for(ReplyKind::Quote),
            (Workflow::WaitingOnPo, WaitingOn::Client)
        );
        assert_eq!(
            transition_for(ReplyKind::Accept),
            (Workflow::Actioned, WaitingOn::Contractor)
        );
    }
}
