//! **Gate 4 — record reach for a dashboard** (nav-reach-record scope). One question, one file: does
//! the caller's nav-derived record reach allow them to open THIS board?
//!
//! Two safety valves live here, and both exist because of a real incident (2026-08-05): a workspace
//! admin carrying a stale tier-1 `/nav/pref` pick at a throwaway one-board nav lost 8 of their 9
//! dashboards — **including boards they owned**. A gate that can take your own work away from you is
//! wrong regardless of what the menu says.
//!
//! 1. **The owner is never gated.** You can always open a dashboard you own. Reach is a *curation*
//!    lens over other people's shared work; it has no business standing between an author and their
//!    own record. This is why the check runs AFTER the fetch — the owner is not knowable from the id.
//! 2. **Only a nav you were HANDED narrows you** (see `nav::reach`): record reach arms for a
//!    team-shared or workspace-default nav, never for the caller's own tier-1 pick. A preference you
//!    set on yourself must not be able to revoke your own access.
//!
//! Together these make the incident structurally impossible: a self-pick no longer arms the gate at
//! all, and even an armed subject keeps everything they authored.

use lb_auth::Principal;

use super::model::Dashboard;
use crate::nav::dashboard_reach_ok;

/// May `principal` open `dashboard` under record reach? True if they own it, or their reach set
/// names it (or record reach is not armed for them at all — the degrade-open default).
///
/// This is a pure NARROWING check layered on top of the visibility gate, never a substitute for it.
pub fn reach_allows(principal: &Principal, ws: &str, dashboard: &Dashboard) -> bool {
    // Valve 1: the author always reaches their own board.
    if principal.owner_sub() == dashboard.owner {
        return true;
    }
    dashboard_reach_ok(principal, ws, &dashboard.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::model::Visibility;

    fn board(id: &str, owner: &str) -> Dashboard {
        Dashboard {
            id: id.into(),
            owner: owner.into(),
            visibility: Visibility::Workspace,
            ..Default::default()
        }
    }

    /// An ARMED subject reaches the board their nav names, and not a workspace-visible one it doesn't.
    #[test]
    fn armed_subject_is_narrowed_to_the_named_board() {
        let p = Principal::routed(
            "user:test",
            "nube",
            vec![
                "reach:dashboards:view".into(),
                "reach:dashboards/__curated__:view".into(),
                "reach:dashboards/demo-analytics:view".into(),
            ],
        );
        // The boards are owned by SOMEONE ELSE — the owner valve must not stand in for reach here.
        assert!(reach_allows(
            &p,
            "nube",
            &board("demo-analytics", "user:bob")
        ));
        assert!(!reach_allows(
            &p,
            "nube",
            &board("modbus-tmpl-sim-meter", "user:bob")
        ));
    }

    /// **Valve 1 — the incident guard.** An armed subject ALWAYS reaches a board they own, even though
    /// their nav does not name it. A curated nav must never take away your own work.
    #[test]
    fn the_owner_is_never_gated_by_reach() {
        let p = Principal::routed(
            "user:test",
            "nube",
            vec![
                "reach:dashboards:view".into(),
                "reach:dashboards/__curated__:view".into(),
                "reach:dashboards/demo-analytics:view".into(),
            ],
        );
        // Not in her reach set, but hers.
        assert!(reach_allows(
            &p,
            "nube",
            &board("demo-plant-report", "user:test")
        ));
        assert!(reach_allows(
            &p,
            "nube",
            &board("demo-analytics-charts-copy", "user:test")
        ));
        // Someone else's, still closed.
        assert!(!reach_allows(
            &p,
            "nube",
            &board("modbus-tmpl-sim-meter", "user:bob")
        ));
    }

    /// An unarmed subject (fallback / legacy / API key) reaches everything, owned or not.
    #[test]
    fn unarmed_subject_reaches_everything() {
        let p = Principal::routed("user:carol", "nube", vec!["reach:*:view".into()]);
        assert!(reach_allows(&p, "nube", &board("anything", "user:test")));
        assert!(reach_allows(&p, "nube", &board("owned", "user:carol")));
    }
}
