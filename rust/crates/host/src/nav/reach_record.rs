//! **Record-granular reach** (nav-reach-record scope): the second half of the nav reach model. The
//! surface half ([`super::reach`]) answers *"may this subject OPEN the Dashboards page?"*; this half
//! answers *"may this subject open THIS dashboard?"*.
//!
//! ## Why it exists
//!
//! A curated nav naming exactly one dashboard used to mint only `reach:dashboards:view` — reach to the
//! whole surface. Within the surface, readability fell back to the record's own visibility, so every
//! `visibility: workspace` board (e.g. an extension's seeded templates) stayed readable. The menu said
//! "one board"; the wall said "every workspace board". This module closes that gap: a curated nav that
//! names dashboards **is** the reach boundary at record granularity.
//!
//! ## The grammar
//!
//! Record reach reuses the `reach` surface with a two-segment resource:
//!
//! ```text
//! reach:dashboards:view                  the SURFACE cap  — may open the Dashboards page
//! reach:dashboards/{id}:view             a RECORD cap     — may open that one board
//! reach:dashboards/__curated__:view      the ARMING cap   — record reach is IN FORCE for this subject
//! ```
//!
//! The arming cap is the whole trick, and it is what makes the gate **degrade open by construction**.
//! [`reach_record_check`] denies only when the subject holds the arming cap *and* lacks the record cap.
//! Every other token — a legacy session, a directly-minted API key, a node/reactor principal, and
//! critically the **fallback** `reach:*:view` — does not hold it, so the record gate is simply not in
//! force for them. Note that the fallback wildcard cannot arm it *by grammar*: `*` spans exactly one
//! segment, and `dashboards/__curated__` is two. That is checked below through the real matcher
//! ([`fallback_wildcard_does_not_arm_record_reach`]) rather than by string inspection — the 2026-07-16
//! `mcp:*.list:call` bug is the precedent for never trusting a `contains()` on a cap string.
//!
//! ## Never widens
//!
//! The gate additionally NEVER closes a board the subject OWNS — that valve lives with the gate, in
//! `dashboard::reach_gate`.
//!
//! Record caps are minted from the ALREADY-resolved nav, whose every dashboard item survived the
//! resolver's three-gate `dashboard.get` strip. So a record cap is only ever emitted for a board the
//! subject could already read, and [`reach_record_check`] is consulted **in addition to** (never
//! instead of) the ordinary visibility gate. A nav naming a board the subject cannot read yields no
//! item, hence no cap, hence no new access.
//!
//! ## Degrade-open escape valves
//!
//! Several cases mint NOTHING (leaving the subject unnarrowed) rather than risk a lockout:
//! - **The nav is the subject's own tier-1 pick.** A preference you set on yourself must never revoke
//!   your own access — the 2026-08-05 incident. Only a menu you were HANDED narrows you.
//! - **The nav names the Dashboards surface itself.** The menu already says "the whole Dashboards
//!   page", so there is no per-record intent to enforce.
//! - **Cardinality / inexpressible id.** Past [`MAX_RECORD_REACH_CAPS`] boards the token would bloat,
//!   and an id carrying a `:` cannot be expressed in a three-part cap string at all. Both fall back to
//!   surface-granular reach (today's behaviour) instead of truncating — a truncated set would silently
//!   lock a subject out of boards their own menu names.

use std::collections::BTreeSet;

use lb_auth::Principal;

use crate::authz::holds_cap;

/// The core surface a `dashboard`-kind nav item opens under. Also the record-reach resource prefix.
pub const DASHBOARD_SURFACE: &str = "dashboards";

/// The sentinel record id whose cap means "record-granular reach is IN FORCE for this surface". Chosen
/// to be un-mintable as a real dashboard id (a store id is a slug; `__curated__` is not one).
const ARMING_ID: &str = "__curated__";

/// Past this many record caps, record reach degrades to surface reach rather than bloating the token.
/// A curated nav of this size is a browse menu, not a restriction, so surface granularity is the
/// honest answer for it.
pub const MAX_RECORD_REACH_CAPS: usize = 128;

/// The `reach:<surface>/<id>:view` cap for one record.
fn record_cap(surface: &str, id: &str) -> String {
    format!("reach:{surface}/{id}:view")
}

/// Derive the record-granular reach caps for `surface` from the resolved nav's `ids`.
///
/// `disarmed` is the caller's "do not narrow this subject at all" signal — true when the nav ALSO
/// names the surface itself (the menu says "the whole page", so there is no per-record intent), and
/// true when the nav is the subject's OWN tier-1 pick rather than one they were handed (see
/// [`super::reach::reach_caps`], valve 2).
///
/// Returns the arming cap plus one cap per id, or an EMPTY vec for every degrade-open case.
pub fn record_reach_caps(surface: &str, ids: &BTreeSet<String>, disarmed: bool) -> Vec<String> {
    if disarmed || ids.is_empty() {
        return Vec::new();
    }
    if ids.len() > MAX_RECORD_REACH_CAPS {
        tracing::warn!(
            surface,
            count = ids.len(),
            max = MAX_RECORD_REACH_CAPS,
            "nav names more records than record-granular reach encodes; degrading to surface reach"
        );
        return Vec::new();
    }
    // An id carrying a `:` cannot round-trip through the three-part cap grammar (`splitn(3, ':')`
    // would land it in the action). Rather than mint a set that silently omits it — which would lock
    // the subject out of a board their OWN menu names — degrade the whole surface open.
    if ids.iter().any(|id| id.contains(':') || id.is_empty()) {
        tracing::warn!(
            surface,
            "nav names a record id inexpressible in the cap grammar; degrading to surface reach"
        );
        return Vec::new();
    }

    let mut caps = Vec::with_capacity(ids.len() + 1);
    caps.push(record_cap(surface, ARMING_ID));
    caps.extend(ids.iter().map(|id| record_cap(surface, id)));
    caps
}

/// **The record reach gate** — may `principal` open record `id` on core `surface` in `ws`?
///
/// True unless record reach is armed for this subject *and* this record is not in their menu. Runs at
/// the same [`holds_cap`] → `lb_caps::check` chokepoint every other cap rides; `surface` and `id` are
/// opaque data (rule 10). This is an ADDITIONAL gate — it never substitutes for the record's own
/// visibility check, so it can only ever subtract.
pub fn reach_record_check(principal: &Principal, ws: &str, surface: &str, id: &str) -> bool {
    // Not armed → record reach is not in force for this token (fallback, legacy, API key, node
    // principal, or a nav that named the whole surface). Degrade OPEN.
    if !holds_cap(principal, ws, &record_cap(surface, ARMING_ID)) {
        return true;
    }
    holds_cap(principal, ws, &record_cap(surface, id))
}

/// Convenience for the dashboard read paths: may `principal` open dashboard `id`?
pub fn dashboard_reach_ok(principal: &Principal, ws: &str, id: &str) -> bool {
    reach_record_check(principal, ws, DASHBOARD_SURFACE, id)
}

#[cfg(test)]
mod tests {
    use super::super::reach::REACH_ALL;
    use super::*;

    fn ids(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// The headline: a nav naming ONE board arms record reach and grants exactly that board.
    #[test]
    fn curated_ids_mint_arming_plus_one_cap_each() {
        let caps = record_reach_caps(DASHBOARD_SURFACE, &ids(&["demo-analytics"]), false);
        assert_eq!(
            caps,
            vec![
                "reach:dashboards/__curated__:view".to_string(),
                "reach:dashboards/demo-analytics:view".to_string(),
            ]
        );
    }

    /// **Guard D (no lockout), grammar half.** The FALLBACK wildcard `reach:*:view` must NOT arm record
    /// reach — probed through the REAL matcher (`holds_cap`), never a string `contains`. `*` spans
    /// exactly one segment and `dashboards/__curated__` is two, so the arming cap is unreachable from
    /// the wildcard. If this ever inverts, every no-curated-nav member loses every dashboard.
    #[test]
    fn fallback_wildcard_does_not_arm_record_reach() {
        let fallback = Principal::routed("user:alice", "nube", vec![REACH_ALL.into()]);
        // The arming probe itself must be denied by the wildcard...
        assert!(!holds_cap(
            &fallback,
            "nube",
            "reach:dashboards/__curated__:view"
        ));
        // ...and therefore every board is reachable.
        for id in ["demo-analytics", "modbus-tmpl-sim-meter", "anything-at-all"] {
            assert!(
                dashboard_reach_ok(&fallback, "nube", id),
                "fallback member must still reach {id}"
            );
        }
    }

    /// **Guard D, token half.** A legacy/API-key token with no reach caps, a node principal, and a
    /// surface-only curated token (pre-record-reach shape) all degrade OPEN.
    #[test]
    fn unarmed_tokens_degrade_open() {
        let legacy = Principal::routed("key:svc", "nube", vec!["mcp:series.list:call".into()]);
        let surface_only =
            Principal::routed("user:bob", "nube", vec!["reach:dashboards:view".into()]);
        for p in [&legacy, &surface_only] {
            for id in ["demo-analytics", "modbus-tmpl-sim-meter"] {
                assert!(dashboard_reach_ok(p, "nube", id), "unarmed must reach {id}");
            }
        }
    }

    /// **The fix.** An ARMED subject reaches the boards their nav names and nothing else — including
    /// workspace-visible boards, which is the entire point.
    #[test]
    fn armed_subject_reaches_only_named_records() {
        let mut caps = record_reach_caps(DASHBOARD_SURFACE, &ids(&["demo-analytics"]), false);
        caps.push("reach:dashboards:view".into());
        let test = Principal::routed("user:test", "nube", caps);

        assert!(dashboard_reach_ok(&test, "nube", "demo-analytics"));
        for denied in [
            "modbus-tmpl-sim-meter",
            "modbus-tmpl-nubeio-io16-current",
            "demo-plant-report",
        ] {
            assert!(
                !dashboard_reach_ok(&test, "nube", denied),
                "{denied} must NOT be reachable"
            );
        }
    }

    /// The record cap must not span the `/` — holding `reach:dashboards/a:view` grants neither the
    /// surface cap nor a sibling record, and the surface cap does not imply a record. Probed through
    /// the real matcher (the `mcp:*.list:call` precedent).
    #[test]
    fn record_cap_does_not_span_segments() {
        let p = Principal::routed(
            "user:bob",
            "nube",
            vec!["reach:dashboards/a:view".into(), "reach:rules:view".into()],
        );
        assert!(holds_cap(&p, "nube", "reach:dashboards/a:view"));
        assert!(!holds_cap(&p, "nube", "reach:dashboards/b:view"));
        assert!(!holds_cap(&p, "nube", "reach:dashboards:view"));
        // ...and a surface cap never grants a record.
        let s = Principal::routed("user:s", "nube", vec!["reach:dashboards:view".into()]);
        assert!(!holds_cap(&s, "nube", "reach:dashboards/a:view"));
    }

    /// Degrade-open cases mint nothing: the nav naming the whole surface, an empty set, an
    /// over-cardinality set, and an id inexpressible in the cap grammar.
    #[test]
    fn degrade_open_cases_mint_nothing() {
        assert!(record_reach_caps(DASHBOARD_SURFACE, &ids(&["a", "b"]), true).is_empty());
        assert!(record_reach_caps(DASHBOARD_SURFACE, &ids(&[]), false).is_empty());

        let many: BTreeSet<String> = (0..=MAX_RECORD_REACH_CAPS)
            .map(|i| format!("board-{i}"))
            .collect();
        assert!(record_reach_caps(DASHBOARD_SURFACE, &many, false).is_empty());

        assert!(record_reach_caps(DASHBOARD_SURFACE, &ids(&["ns:board"]), false).is_empty());
    }

    /// A dotted id stays ONE literal pattern that matches itself and nothing adjacent. (`.` is a
    /// segment delimiter in the matcher, so `a.b` is two segments — the literal still round-trips, but
    /// a `*` would not cover it. Record caps are always literal, so this holds.)
    #[test]
    fn dotted_record_id_round_trips() {
        let caps = record_reach_caps(DASHBOARD_SURFACE, &ids(&["site.a"]), false);
        let p = Principal::routed("user:bob", "nube", caps);
        assert!(dashboard_reach_ok(&p, "nube", "site.a"));
        assert!(!dashboard_reach_ok(&p, "nube", "site.b"));
        assert!(!dashboard_reach_ok(&p, "nube", "site"));
    }

    /// Workspace isolation still runs first: an armed subject's record cap does not cross workspaces.
    #[test]
    fn record_reach_is_workspace_scoped() {
        let caps = record_reach_caps(DASHBOARD_SURFACE, &ids(&["demo-analytics"]), false);
        let p = Principal::routed("user:test", "nube", caps);
        assert!(dashboard_reach_ok(&p, "nube", "demo-analytics"));
        // A different ws fails gate 1, so even the ARMING probe is denied → degrade open. Reach is a
        // narrowing lens, not the isolation wall; `check`'s gate 1 remains the wall for other-ws reads.
        assert!(dashboard_reach_ok(&p, "other", "demo-analytics"));
    }
}
