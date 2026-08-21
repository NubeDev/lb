//! What the CLIENT actually rendered, and how it becomes something [`super::compose`] can place.
//!
//! This is the input half of "the PDF follows the PAGE, not the record". The exporter used to lay out
//! from the stored `cells`, which is a different list in four ways an author hits routinely — a row
//! header (section chrome, never capturable), a panel a filter's `showWhen` hid, a parked cell or a
//! collapsed row's member, and a repeat clone (whose derived key is in no record at all). Every one of
//! them showed up in the PDF as a full-width dashed *"not captured"* tile, or — for the clone — as one
//! such tile standing in for the N real panels whose captures were being discarded.
//!
//! So the client sends one entry per panel it drew, carrying the grid rect it drew it at, and the
//! layout is computed over [`Placed`] rather than over `Cell`. That indirection is the point: a repeat
//! clone has no `Cell` anywhere, and inventing one would mean inventing its spec too.
//!
//! One responsibility: the layout's input vocabulary.

use crate::dashboard::Cell;

/// One panel the CLIENT reports having rendered: its cell key, the PNG it captured (empty when it could
/// not be rasterised), the grid rect it was drawn at, and the reason it failed, if it did.
///
/// A zero-area rect means "no rendered geometry" — an older client that sends only `(cellId, png)`. Such
/// an entry resolves its rect from the record instead, which is exactly the shipped behaviour.
#[derive(Debug, Clone, Default)]
pub struct RenderedPanel {
    /// The cell key. For a repeat clone this is the DERIVED key, which is in no record.
    pub cell_id: String,
    /// The capture. Empty ⇒ nothing to draw; an error tile is placed instead.
    pub png: Vec<u8>,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// Why the capture is empty, when the client knows. Empty ⇒ the generic note.
    pub reason: String,
}

impl RenderedPanel {
    /// Did the client tell us where this panel was drawn? A zero-area rect is the older wire shape.
    pub(super) fn has_rect(&self) -> bool {
        self.w > 0 && self.h > 0
    }
}

/// One thing to place: a rect on the board and the title to label it with. The layout is computed over
/// these rather than over `Cell` so the client's rendered list and the record can drive the same code —
/// a repeat clone has no `Cell` anywhere, and inventing one would mean inventing its spec too.
pub(super) struct Placed {
    pub id: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub title: String,
}

/// The title for a rendered panel. An exact cell match first; failing that, a repeat clone's derived
/// key (`{source}-clone-{n}`) resolves to the cell it was cloned from, so the N tiles of a repeat carry
/// the panel's name instead of going blank. Anything else is untitled — never guessed.
pub(super) fn title_for(cells: &[Cell], id: &str) -> String {
    if let Some(c) = cells.iter().find(|c| c.i == id) {
        return c.title.clone();
    }
    id.rsplit_once("-clone-")
        .and_then(|(source, n)| n.parse::<u32>().ok().map(|_| source))
        .and_then(|source| cells.iter().find(|c| c.i == source))
        .map(|c| c.title.clone())
        .unwrap_or_default()
}

/// The list to lay the PDF out over.
///
/// When the client reported rendered geometry, THAT is the page — one entry per panel it drew, at the
/// rect it drew it. Otherwise fall back to the record: an older client, and the only way a caller with
/// no browser at all can compose. Keeping the fallback is what makes the wire change additive rather
/// than a flag day for the headless render worker.
pub(super) fn panels_to_place(cells: &[Cell], panels: &[RenderedPanel]) -> Vec<Placed> {
    // WHAT TO LAY OUT. When the client reported rendered geometry, that list IS the page — one entry
    // per panel it drew, at the rect it drew it. Otherwise fall back to the record (an older client),
    // which is the shipped behaviour and the only way a caller with no browser can compose at all.
    if panels.iter().any(RenderedPanel::has_rect) {
        panels
            .iter()
            .filter(|p| p.has_rect())
            .map(|p| Placed {
                id: p.cell_id.clone(),
                x: p.x,
                y: p.y,
                w: p.w,
                h: p.h,
                // A clone is in no record, so its title comes from the cell it was cloned FROM when we
                // can find one; the export never invents a name it was not given.
                title: title_for(cells, &p.cell_id),
            })
            .collect()
    } else {
        cells
            .iter()
            .map(|c| Placed {
                id: c.i.clone(),
                x: c.x,
                y: c.y,
                w: c.w,
                h: c.h,
                title: c.title.clone(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(i: &str, title: &str) -> Cell {
        Cell {
            i: i.into(),
            title: title.into(),
            ..Cell::default()
        }
    }

    #[test]
    fn a_zero_area_rect_reads_as_no_rendered_geometry() {
        // The older wire shape — `(cellId, png)` and nothing else — must not be mistaken for a panel
        // that genuinely rendered at 0x0, or the record fallback would never trigger.
        assert!(!RenderedPanel::default().has_rect());
        assert!(!RenderedPanel {
            w: 6,
            ..RenderedPanel::default()
        }
        .has_rect());
        assert!(RenderedPanel {
            w: 6,
            h: 5,
            ..RenderedPanel::default()
        }
        .has_rect());
    }

    #[test]
    fn a_clone_key_resolves_to_the_panel_it_was_cloned_from() {
        // `{source}-clone-{n}` is in no record, so without this a repeat's tiles would all go untitled.
        let cells = [cell("meter", "Meter")];
        assert_eq!(title_for(&cells, "meter"), "Meter");
        assert_eq!(title_for(&cells, "meter-clone-0"), "Meter");
        assert_eq!(title_for(&cells, "meter-clone-11"), "Meter");
    }

    #[test]
    fn a_key_that_merely_looks_like_a_clone_is_not_given_someone_elses_title() {
        let cells = [cell("meter", "Meter"), cell("meter-clone-x", "Literal")];
        // A cell whose own key contains the marker matches EXACTLY first, and keeps its own title.
        assert_eq!(title_for(&cells, "meter-clone-x"), "Literal");
        // A non-numeric suffix is not a clone index, so nothing is inherited.
        assert_eq!(title_for(&[cell("meter", "Meter")], "meter-clone-x"), "");
        // An unknown source is untitled, never guessed.
        assert_eq!(title_for(&cells, "ghost-clone-0"), "");
    }
}
