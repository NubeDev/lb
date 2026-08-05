//! **Nav → reach caps** (nav-reach scope): derive the `reach:<surface>:view` capabilities a subject's
//! resolved nav grants. This is the *narrowing* half of the nav model — the lens never *widens* reach
//! (it can't grant a data cap), but a **curated** nav now *gates* reach: one page in the nav ⇒ that is
//! the only core surface the subject may OPEN. The reach caps are minted into the token at login (the
//! `resolve_caps` fold on the `login` route), and each core surface's entry route re-checks
//! `reach:<surface>:view` at the same `lb_caps::check` choke point every other cap rides.
//!
//! **The fallback is load-bearing.** `nav.resolve` returns [`ResolvedSource::Fallback`] when *no nav
//! applies* — the state of every existing member/admin who never authored a custom nav. Deriving an
//! empty reach set for them would lock them out of everything. So a **fallback yields the wildcard
//! `reach:*:view`** (the grammar's single-segment `*` grants any surface), and the reach gate bites
//! **only** for a subject handed an explicit, curated nav. This keeps the default open and makes the
//! restriction strictly opt-in (you get it by being *given a menu*, never by default).
//!
//! **No-widening (rule preserved).** The derivation reads the ALREADY-resolved nav — every item has
//! survived the resolver's cap-strip (`resolve.rs`). So reach is only ever emitted for a surface the
//! caller could already reach by cap; the nav can *subtract* reachable surfaces, never *add* one.
//!
//! **Rule 10.** The surface key is opaque `ResolvedItem.surface`/`.dashboard` data carried straight
//! from the nav; nothing here branches on a page or ext id. `ext` items map to NO core reach cap —
//! extension reach stays the opaque `ext.list` install seam (the resolver already strips uninstalled
//! exts), so this deliberately ignores `ext` kinds.

use std::collections::BTreeSet;

use lb_auth::Principal;

use super::model::{ResolvedItem, ResolvedNav, ResolvedSource};
use super::reach_record::{record_reach_caps, DASHBOARD_SURFACE};
use crate::authz::holds_cap;

/// The wildcard reach cap a fallback (no-curated-nav) subject holds — reaches every core surface, so a
/// default member/admin is never locked out by a nav they never authored.
pub const REACH_ALL: &str = "reach:*:view";

/// What a resolved nav reaches: the core surfaces it names, and — separately — the individual
/// dashboard records it names. The split is what makes record granularity possible: a nav that names
/// the `dashboards` SURFACE expresses "the whole page", while a nav that names only dashboard ITEMS
/// expresses "these boards" (see [`super::reach_record`]).
#[derive(Default)]
struct Reached {
    surfaces: BTreeSet<String>,
    dashboards: BTreeSet<String>,
}

/// Derive the reach caps `resolved` grants, sorted + deduped.
///
/// - **Fallback** (no curated nav) → `[reach:*:view]` — reaches all (never locked out). No record caps,
///   so record reach is never armed for a fallback subject.
/// - **Curated** (pick / team / workspace-default) → one `reach:<surface>:view` per distinct core
///   surface the menu (and the caller's pins) reach, walking `group` children.
/// - **Handed** (team / workspace-default ONLY) → additionally the record-granular
///   `reach:dashboards/{id}:view` caps, when the menu names dashboards but not the Dashboards
///   surface itself (nav-reach-record scope).
///
/// **A tier-1 PICK never arms record reach.** Surface narrowing from your own pick is the shipped
/// behaviour and stays; record narrowing does not join it. The reason is an incident (2026-08-05): an
/// admin's stale self-pick at a one-board nav took away 8 of their 9 dashboards. A preference you set
/// on yourself must not be able to revoke your own access — and per this scope's own Non-goals a pick
/// cannot confine anyone anyway (clearing it is one click), so arming on it bought no restriction and
/// cost a foot-gun. Record reach therefore arms only for a menu you were HANDED.
///
/// The result is unioned into the token alongside the caller's other caps; the surface entry routes
/// then require the matching `reach:<surface>:view` (or the wildcard) to open a page, and
/// `dashboard.get`/`dashboard.list` require the matching record cap to open a board.
pub fn reach_caps(resolved: &ResolvedNav) -> Vec<String> {
    // A fallback nav reaches everything — the gate only restricts an explicitly curated menu.
    if resolved.source == ResolvedSource::Fallback {
        return vec![REACH_ALL.to_string()];
    }

    let mut reached = Reached::default();
    for item in &resolved.items {
        collect(item, &mut reached);
    }
    // Pins are personal shortcuts resolved through the same cap-strip pipeline — a surface the caller
    // pinned is one they can reach, so it counts toward reach (it can never widen: a pin only survives
    // if the caller already holds the surface's data cap). A pinned BOARD likewise counts as a named
    // record — otherwise curating a nav would silently break the caller's own pins.
    for pin in &resolved.pinned {
        collect(pin, &mut reached);
    }

    // A named dashboard reaches the Dashboards page, but only the record caps below open the board.
    let whole_dashboards_surface = reached.surfaces.contains(DASHBOARD_SURFACE);
    if !reached.dashboards.is_empty() {
        reached.surfaces.insert(DASHBOARD_SURFACE.to_string());
    }

    // Valve 2: record reach arms only for a menu the subject was HANDED (a team share or the
    // workspace default), never for their own tier-1 pick. See the doc comment above.
    let handed = matches!(
        resolved.source,
        ResolvedSource::Team | ResolvedSource::WorkspaceDefault
    );

    let mut caps: Vec<String> = reached
        .surfaces
        .iter()
        .map(|s| format!("reach:{s}:view"))
        .collect();
    caps.extend(record_reach_caps(
        DASHBOARD_SURFACE,
        &reached.dashboards,
        whole_dashboards_surface || !handed,
    ));
    caps.sort();
    caps.dedup();
    caps
}

/// **The reach gate** — may `principal` OPEN the core `surface` (page) in `ws`? True iff they hold
/// `reach:<surface>:view` (or the fallback wildcard `reach:*:view`). Called at each core surface's
/// ENTRY route, keyed on the surface (NOT the entry verb) so the two gate-cap-vs-entry-read mismatches
/// (rules: gate `rules.run` / entry `rules.list`; data: gate `store.scan` / entry `store.query`) don't
/// matter. `surface` is opaque data (rule 10). Every real human token is minted with at least
/// `reach:*:view` (fallback) or the curated set; a token with NO `reach:` cap at all degrades OPEN (see
/// below). Composes at the same `lb_caps::check` primitive every other cap rides — no new choke.
pub fn reach_check(principal: &Principal, ws: &str, surface: &str) -> bool {
    if holds_cap(principal, ws, &format!("reach:{surface}:view")) {
        return true;
    }
    // Degrade OPEN when the token carries NO `reach:` cap at all — a legacy token minted before this
    // feature, or a directly-minted credential (an API key) whose reach set was never folded. We never
    // deny on the mere ABSENCE of reach data (that would lock out every pre-existing session); we only
    // deny when reach data is PRESENT and says no. This mirrors the login degrade-open (a nav-resolve
    // error folds `reach:*:view`) — a real curated session always carries concrete reach caps, so this
    // touches only the "reach unknown" case, never the "reach says no" case. `reach:*:view` (fallback)
    // is a present reach cap that `holds_cap` above already granted, so it never reaches this branch.
    !principal.caps().iter().any(|c| c.starts_with("reach:"))
}

/// Accumulate what one resolved item reaches. A `surface` item maps to its key; a `dashboard` item
/// maps to its RECORD id (and, via the caller, the `dashboards` surface); a `group` (author group /
/// expanded tag-group / expanded template-group) recurses into its children. `ext` and empty kinds map
/// to no core reach cap (ext reach is the `ext.list` seam — rule 10).
///
/// The `dashboard` field carries a `dashboard:{id}` reference (the resolver normalises to that form);
/// the bare id is what the record cap names.
fn collect(item: &ResolvedItem, out: &mut Reached) {
    match item.kind.as_str() {
        "surface" if !item.surface.is_empty() => {
            out.surfaces.insert(item.surface.clone());
        }
        "dashboard" if !item.dashboard.is_empty() => {
            let id = item
                .dashboard
                .strip_prefix("dashboard:")
                .unwrap_or(&item.dashboard);
            out.dashboards.insert(id.to_string());
        }
        "group" => {
            for child in &item.items {
                collect(child, out);
            }
        }
        // `ext` (opaque-id reach via ext.list) or anything else — no core reach cap.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface_item(key: &str) -> ResolvedItem {
        ResolvedItem {
            kind: "surface".into(),
            label: key.into(),
            icon: String::new(),
            surface: key.into(),
            dashboard: String::new(),
            ext: String::new(),
            items: Vec::new(),
            vars: Default::default(),
            ..Default::default()
        }
    }

    fn nav(source: ResolvedSource, items: Vec<ResolvedItem>) -> ResolvedNav {
        ResolvedNav {
            source,
            nav_id: String::new(),
            title: String::new(),
            items,
            hidden: Vec::new(),
            pinned: Vec::new(),
        }
    }

    /// A FALLBACK nav (no curated menu) yields the wildcard — reaches all. This is the
    /// catastrophic-regression guard: a default member/admin must NOT be locked out.
    #[test]
    fn fallback_yields_wildcard_reach_all() {
        let resolved = nav(ResolvedSource::Fallback, Vec::new());
        assert_eq!(reach_caps(&resolved), vec![REACH_ALL.to_string()]);
    }

    /// A curated one-page nav yields EXACTLY that surface's reach cap — and no other. This is the
    /// headline: one page in the nav ⇒ one reachable surface.
    #[test]
    fn curated_one_page_yields_only_that_surface() {
        let resolved = nav(ResolvedSource::Pick, vec![surface_item("dashboards")]);
        assert_eq!(
            reach_caps(&resolved),
            vec!["reach:dashboards:view".to_string()]
        );
        // …and crucially NOT the wildcard, so rules/flows/ingest are all denied.
        assert!(!reach_caps(&resolved).contains(&REACH_ALL.to_string()));
    }

    /// A `dashboard` item grants reach to the `dashboards` surface (a dashboard page renders there)
    /// AND record-granular reach to exactly that board — the arming cap plus one record cap
    /// (nav-reach-record scope). The `dashboard:` prefix is stripped: the record cap names the bare id.
    #[test]
    fn dashboard_item_grants_dashboards_surface() {
        let dash = ResolvedItem {
            kind: "dashboard".into(),
            label: "Site health".into(),
            icon: String::new(),
            surface: String::new(),
            dashboard: "dashboard:site-health".into(),
            ext: String::new(),
            items: Vec::new(),
            vars: Default::default(),
            ..Default::default()
        };
        let resolved = nav(ResolvedSource::Team, vec![dash]);
        assert_eq!(
            reach_caps(&resolved),
            vec![
                "reach:dashboards/__curated__:view".to_string(),
                "reach:dashboards/site-health:view".to_string(),
                "reach:dashboards:view".to_string(),
            ]
        );
    }

    /// **The reported bug (B), at the derivation layer.** A team nav naming ONE board mints reach for
    /// that board and NO other — so a workspace-visible board nobody put in the menu is closed.
    /// Asserted through the real matcher, not by string inspection.
    #[test]
    fn curated_dashboard_nav_closes_unnamed_workspace_boards() {
        let dash = ResolvedItem {
            kind: "dashboard".into(),
            dashboard: "dashboard:demo-analytics".into(),
            ..Default::default()
        };
        let caps = reach_caps(&nav(ResolvedSource::Team, vec![dash]));
        let test = Principal::routed("user:test", "acme", caps);

        assert!(reach_check(&test, "acme", "dashboards"), "the PAGE is open");
        assert!(super::super::dashboard_reach_ok(
            &test,
            "acme",
            "demo-analytics"
        ));
        for board in ["modbus-tmpl-sim-meter", "modbus-tmpl-nubeio-io16-current"] {
            assert!(
                !super::super::dashboard_reach_ok(&test, "acme", board),
                "{board} is workspace-visible but NOT in the nav — must be closed"
            );
        }
    }

    /// **Guard D at the derivation layer.** A FALLBACK nav mints no record caps, so record reach is
    /// never armed and every board stays reachable. This is the lockout guard.
    #[test]
    fn fallback_never_arms_record_reach() {
        let caps = reach_caps(&nav(ResolvedSource::Fallback, Vec::new()));
        assert_eq!(caps, vec![REACH_ALL.to_string()]);
        let member = Principal::routed("user:alice", "acme", caps);
        for board in ["demo-analytics", "modbus-tmpl-sim-meter", "whatever"] {
            assert!(super::super::dashboard_reach_ok(&member, "acme", board));
        }
    }

    /// **Guard F (no widening).** Derivation reads the ALREADY-resolved nav, whose items survived the
    /// resolver's three-gate `dashboard.get` strip. A nav naming a board the subject cannot read
    /// therefore resolves to NO item, so no record cap is minted and nothing becomes readable. A cap
    /// can only ever be emitted for a board already present in the resolved menu.
    #[test]
    fn unreadable_board_stripped_by_resolver_mints_no_cap() {
        // The resolver dropped the unreadable board; only the readable one survived into `items`.
        let readable = ResolvedItem {
            kind: "dashboard".into(),
            dashboard: "dashboard:demo-analytics".into(),
            ..Default::default()
        };
        let caps = reach_caps(&nav(ResolvedSource::Team, vec![readable]));
        assert!(!caps
            .iter()
            .any(|c| c.contains("someone-elses-private-board")));
        // And reach is not a grant: holding the record cap is necessary, never sufficient — gate 3
        // (`may_read_dashboard`) still runs after it in `dashboard_get`.
        let test = Principal::routed("user:test", "acme", caps);
        assert!(!super::super::dashboard_reach_ok(
            &test,
            "acme",
            "someone-elses-private-board"
        ));
    }

    /// A nav naming the Dashboards SURFACE itself expresses "the whole page" — no record narrowing,
    /// so record reach stays unarmed even alongside a named board.
    #[test]
    fn surface_item_disarms_record_reach() {
        let dash = ResolvedItem {
            kind: "dashboard".into(),
            dashboard: "dashboard:demo-analytics".into(),
            ..Default::default()
        };
        let caps = reach_caps(&nav(
            ResolvedSource::Team,
            vec![surface_item("dashboards"), dash],
        ));
        assert_eq!(caps, vec!["reach:dashboards:view".to_string()]);
        let p = Principal::routed("user:bob", "acme", caps);
        assert!(super::super::dashboard_reach_ok(
            &p,
            "acme",
            "modbus-tmpl-sim-meter"
        ));
    }

    /// **Valve 2 — the incident guard.** A tier-1 PICK never arms record reach, so a stale self-pick
    /// at a one-board nav cannot take away the subject's dashboards. The SURFACE narrowing from a pick
    /// is unchanged (that is the shipped behaviour); only the record narrowing is withheld.
    ///
    /// This is the exact 2026-08-05 shape: an admin whose `/nav/pref` pointed at a throwaway one-board
    /// nav lost 8 of 9 boards, including ones they owned.
    #[test]
    fn a_self_pick_never_arms_record_reach() {
        let dash = ResolvedItem {
            kind: "dashboard".into(),
            dashboard: "dashboard:demo-analytics".into(),
            ..Default::default()
        };
        let caps = reach_caps(&nav(ResolvedSource::Pick, vec![dash.clone()]));
        // The surface cap is still derived — a pick DOES narrow which pages you see.
        assert_eq!(caps, vec!["reach:dashboards:view".to_string()]);
        // ...but no record cap, so every board stays reachable.
        let p = Principal::routed("user:ada", "acme", caps);
        for board in [
            "demo-analytics",
            "modbus-tmpl-sim-meter",
            "demo-plant-report",
        ] {
            assert!(
                super::super::dashboard_reach_ok(&p, "acme", board),
                "a self-pick must not close {board}"
            );
        }

        // The SAME nav, handed to them as a team share, DOES arm.
        let handed = reach_caps(&nav(ResolvedSource::Team, vec![dash]));
        let h = Principal::routed("user:test", "acme", handed);
        assert!(super::super::dashboard_reach_ok(
            &h,
            "acme",
            "demo-analytics"
        ));
        assert!(!super::super::dashboard_reach_ok(
            &h,
            "acme",
            "modbus-tmpl-sim-meter"
        ));
    }

    /// The workspace default is also a menu you were HANDED, so it arms.
    #[test]
    fn workspace_default_arms_record_reach() {
        let dash = ResolvedItem {
            kind: "dashboard".into(),
            dashboard: "dashboard:demo-analytics".into(),
            ..Default::default()
        };
        let caps = reach_caps(&nav(ResolvedSource::WorkspaceDefault, vec![dash]));
        let p = Principal::routed("user:test", "acme", caps);
        assert!(super::super::dashboard_reach_ok(
            &p,
            "acme",
            "demo-analytics"
        ));
        assert!(!super::super::dashboard_reach_ok(
            &p,
            "acme",
            "modbus-tmpl-sim-meter"
        ));
    }

    /// A pinned board counts as a named record — curating a nav must not silently break the caller's
    /// own pins.
    #[test]
    fn pinned_dashboard_contributes_record_reach() {
        let mut resolved = nav(ResolvedSource::Team, vec![]);
        resolved.items = vec![ResolvedItem {
            kind: "dashboard".into(),
            dashboard: "dashboard:demo-analytics".into(),
            ..Default::default()
        }];
        resolved.pinned = vec![ResolvedItem {
            kind: "dashboard".into(),
            dashboard: "dashboard:my-pin".into(),
            ..Default::default()
        }];
        let p = Principal::routed("user:bob", "acme", reach_caps(&resolved));
        assert!(super::super::dashboard_reach_ok(&p, "acme", "my-pin"));
        assert!(super::super::dashboard_reach_ok(
            &p,
            "acme",
            "demo-analytics"
        ));
        assert!(!super::super::dashboard_reach_ok(&p, "acme", "other"));
    }

    /// A `group` recurses one level; an `ext` child contributes NO core reach cap (rule 10 — ext reach
    /// is the `ext.list` seam). Surfaces are deduped + sorted.
    #[test]
    fn group_recurses_and_ext_is_ignored() {
        let ext_child = ResolvedItem {
            kind: "ext".into(),
            label: "mqtt".into(),
            icon: String::new(),
            surface: String::new(),
            dashboard: String::new(),
            ext: "mqtt".into(),
            items: Vec::new(),
            vars: Default::default(),
            ..Default::default()
        };
        let group = ResolvedItem {
            kind: "group".into(),
            label: "Ops".into(),
            icon: String::new(),
            surface: String::new(),
            dashboard: String::new(),
            ext: String::new(),
            items: vec![surface_item("flows"), surface_item("rules"), ext_child],
            vars: Default::default(),
            ..Default::default()
        };
        let resolved = nav(ResolvedSource::WorkspaceDefault, vec![group]);
        assert_eq!(
            reach_caps(&resolved),
            vec![
                "reach:flows:view".to_string(),
                "reach:rules:view".to_string()
            ]
        );
    }

    /// `reach_check`: a token with concrete reach caps reaches ONLY those surfaces; a token with the
    /// wildcard reaches all; a token with NO reach cap degrades OPEN (reach unknown ≠ reach denied).
    #[test]
    fn reach_check_enforces_present_reach_and_degrades_open_on_absence() {
        let curated = Principal::routed("user:bob", "acme", vec!["reach:dashboards:view".into()]);
        assert!(reach_check(&curated, "acme", "dashboards"));
        assert!(!reach_check(&curated, "acme", "rules"));
        assert!(!reach_check(&curated, "acme", "ingest"));

        let fallback = Principal::routed("user:alice", "acme", vec![REACH_ALL.into()]);
        for s in ["dashboards", "rules", "ingest", "system"] {
            assert!(reach_check(&fallback, "acme", s), "wildcard reaches {s}");
        }

        // A token with data caps but NO reach cap at all — reach unknown, degrade open (legacy/API key).
        let no_reach = Principal::routed("key:svc", "acme", vec!["mcp:series.list:call".into()]);
        for s in ["dashboards", "rules", "ingest"] {
            assert!(
                reach_check(&no_reach, "acme", s),
                "a token with no reach cap degrades open for {s}"
            );
        }
    }

    /// Pins count toward reach (a surface the caller pinned is one they can reach), deduped with the
    /// menu items.
    #[test]
    fn pins_contribute_reach() {
        let mut resolved = nav(ResolvedSource::Pick, vec![surface_item("dashboards")]);
        resolved.pinned = vec![surface_item("telemetry")];
        assert_eq!(
            reach_caps(&resolved),
            vec![
                "reach:dashboards:view".to_string(),
                "reach:telemetry:view".to_string(),
            ]
        );
    }
}
