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
use crate::authz::holds_cap;

/// The wildcard reach cap a fallback (no-curated-nav) subject holds — reaches every core surface, so a
/// default member/admin is never locked out by a nav they never authored.
pub const REACH_ALL: &str = "reach:*:view";

/// The core surface a `dashboard`-kind nav item opens under — a dashboard page renders on the
/// `dashboards` surface (UI: `selectDashboard` navigates to `fullPathForSurface(ws, "dashboards")`), so
/// a nav that grants a dashboard grants reach to the Dashboards surface.
const DASHBOARD_SURFACE: &str = "dashboards";

/// Derive the `reach:<surface>:view` caps `resolved` grants, sorted + deduped.
///
/// - **Fallback** (no curated nav) → `[reach:*:view]` — reaches all (never locked out).
/// - **Curated** (pick / team / workspace-default) → one `reach:<surface>:view` per distinct core
///   surface the menu (and the caller's pins) reach, walking `group` children one level.
///
/// The result is unioned into the token alongside the caller's other caps; the surface entry routes
/// then require the matching `reach:<surface>:view` (or the wildcard) to open a page.
pub fn reach_caps(resolved: &ResolvedNav) -> Vec<String> {
    // A fallback nav reaches everything — the gate only restricts an explicitly curated menu.
    if resolved.source == ResolvedSource::Fallback {
        return vec![REACH_ALL.to_string()];
    }

    let mut surfaces: BTreeSet<String> = BTreeSet::new();
    for item in &resolved.items {
        collect_surfaces(item, &mut surfaces);
    }
    // Pins are personal shortcuts resolved through the same cap-strip pipeline — a surface the caller
    // pinned is one they can reach, so it counts toward reach (it can never widen: a pin only survives
    // if the caller already holds the surface's data cap).
    for pin in &resolved.pinned {
        collect_surfaces(pin, &mut surfaces);
    }

    surfaces
        .into_iter()
        .map(|s| format!("reach:{s}:view"))
        .collect()
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

/// Accumulate the core surface(s) one resolved item reaches. A `surface` item maps to its key; a
/// `dashboard` item maps to the `dashboards` surface; a `group` (author group / expanded tag-group /
/// expanded template-group) recurses one level into its children. `ext` and empty kinds map to no core
/// reach cap (ext reach is the `ext.list` seam — rule 10).
fn collect_surfaces(item: &ResolvedItem, out: &mut BTreeSet<String>) {
    match item.kind.as_str() {
        "surface" if !item.surface.is_empty() => {
            out.insert(item.surface.clone());
        }
        "dashboard" if !item.dashboard.is_empty() => {
            out.insert(DASHBOARD_SURFACE.to_string());
        }
        "group" => {
            for child in &item.items {
                collect_surfaces(child, out);
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

    /// A `dashboard` item grants reach to the `dashboards` surface (a dashboard page renders there).
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
        };
        let resolved = nav(ResolvedSource::Team, vec![dash]);
        assert_eq!(
            reach_caps(&resolved),
            vec!["reach:dashboards:view".to_string()]
        );
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
