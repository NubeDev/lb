//! The dashboard's **library-panel closure** — the one enumeration of "which panels does this page
//! embed?" (share-closure scope, "What reuse means concretely").
//!
//! Two verbs ask this question and MUST get the same answer:
//!   - `dashboard.access_check` — "can the SUBJECT read each of them?" (the detection dual).
//!   - `dashboard.share_closure` — "can the CALLER share each of them to a team?" (the remediation).
//!
//! They ask about different principals, so they cannot share a verdict walk — but if they enumerated
//! the closure *separately* they could drift about what the closure even IS, and a share_closure that
//! walks fewer cells than access_check would report "all shared" while a panel stays private: the
//! false-green the whole model exists to prevent. So the enumeration lives here, once, and both call
//! it. The dual-consistency test pins that they agree.
//!
//! **Depth (v1), mirroring `access_check`.** Only DIRECT `panel_ref` cells are `Walked`. A panel's own
//! nested panel refs are a hop v1 does not walk: reported [`PanelRef::unchecked`], never silently
//! shared (which would widen a panel the caller never previewed) and never silently dropped (which
//! would false-green a still-broken closure). Cycles/duplicates collapse to one entry per panel id —
//! a panel embedded twice on a page is ONE asset with ONE audience, so it gets one row, not two.

use std::collections::BTreeSet;

use super::model::Dashboard;

/// One library panel a dashboard embeds — the unit both verbs walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelRef {
    /// The bare panel id (no `panel:` prefix) — the key `read_panel` takes.
    pub id: String,
    /// The FIRST cell (`Cell.i`) that references it, so a report can point the UI at a tile. A panel
    /// embedded in several cells reports the first; it is one asset either way.
    pub cell: String,
    /// True for a hop v1 does not resolve (nested panel→panel). The caller must report it as
    /// `unchecked` and act on NEITHER side — never share it, never call it green.
    pub unchecked: bool,
}

impl PanelRef {
    fn walked(id: impl Into<String>, cell: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            cell: cell.into(),
            unchecked: false,
        }
    }
}

/// Enumerate the library panels `dashboard` embeds, in cell order, de-duplicated by panel id.
///
/// v1 depth: direct `panel_ref` cells only. The returned refs are the closure BOTH `access_check` and
/// `share_closure` reason over — neither may re-derive it (rule 9: no parallel walk).
pub fn closure_panels(dashboard: &Dashboard) -> Vec<PanelRef> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut refs: Vec<PanelRef> = Vec::new();
    for cell in &dashboard.cells {
        if cell.panel_ref.is_empty() {
            continue;
        }
        // Tolerate both the `panel:<id>` and bare-`<id>` forms a stored cell may carry (the ref is
        // written by clients; `access_check` trims the same prefix).
        let id = cell.panel_ref.trim_start_matches("panel:");
        if id.is_empty() {
            continue;
        }
        // One row per panel: the same asset embedded twice has one owner and one audience.
        if !seen.insert(id.to_string()) {
            continue;
        }
        refs.push(PanelRef::walked(id, &cell.i));
    }
    refs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dashboard::model::Cell;

    fn ref_cell(i: &str, panel_ref: &str) -> Cell {
        Cell {
            i: i.into(),
            panel_ref: panel_ref.into(),
            v: 3,
            ..Default::default()
        }
    }

    fn inline_cell(i: &str) -> Cell {
        Cell {
            i: i.into(),
            view: "chart".into(),
            v: 3,
            ..Default::default()
        }
    }

    /// A minimal dashboard carrying `cells` — the enumeration reads nothing else.
    fn dash(cells: Vec<Cell>) -> Dashboard {
        Dashboard {
            id: "d".into(),
            title: "D".into(),
            description: String::new(),
            icon: String::new(),
            color: String::new(),
            toolbar: Default::default(),
            timezone: String::new(),
            owner: "user:ada".into(),
            visibility: Default::default(),
            cells,
            variables: Vec::new(),
            schema_version: 3,
            updated_ts: 0,
            deleted: false,
        }
    }

    /// The closure is the ref cells, in order, with the referencing cell recorded — inline cells
    /// contribute nothing (they embed no library panel).
    #[test]
    fn enumerates_ref_cells_and_ignores_inline_cells() {
        let d = dash(vec![
            ref_cell("a", "panel:cpu"),
            inline_cell("b"),
            ref_cell("c", "panel:mem"),
        ]);
        let refs = closure_panels(&d);
        assert_eq!(refs.len(), 2, "only the two ref cells are in the closure");
        assert_eq!(refs[0], PanelRef::walked("cpu", "a"));
        assert_eq!(refs[1], PanelRef::walked("mem", "c"));
    }

    /// Both the `panel:<id>` and bare `<id>` ref forms resolve to the same panel id.
    #[test]
    fn tolerates_both_ref_forms() {
        let d = dash(vec![ref_cell("a", "panel:cpu"), ref_cell("b", "mem")]);
        let refs = closure_panels(&d);
        assert_eq!(refs[0].id, "cpu");
        assert_eq!(refs[1].id, "mem");
    }

    /// The SAME panel embedded in two cells is ONE closure entry — one asset, one owner, one
    /// audience. Sharing it twice would be a second identical edge write and a duplicated report row.
    #[test]
    fn dedupes_a_panel_embedded_twice() {
        let d = dash(vec![
            ref_cell("a", "panel:cpu"),
            ref_cell("b", "panel:cpu"),
            ref_cell("c", "panel:mem"),
        ]);
        let refs = closure_panels(&d);
        assert_eq!(refs.len(), 2, "cpu collapses to one entry");
        assert_eq!(
            refs[0],
            PanelRef::walked("cpu", "a"),
            "keeps the FIRST cell"
        );
        assert_eq!(refs[1].id, "mem");
    }

    /// An empty/whitespace-only ref is not a panel — never a closure entry (it would become a
    /// `panel:` read of the empty id).
    #[test]
    fn skips_an_empty_ref() {
        let d = dash(vec![ref_cell("a", ""), ref_cell("b", "panel:")]);
        assert!(closure_panels(&d).is_empty());
    }

    /// A page with no cells has an empty closure (the "nothing to share" case the UI must not nag on).
    #[test]
    fn empty_dashboard_has_an_empty_closure() {
        assert!(closure_panels(&dash(vec![])).is_empty());
    }
}
