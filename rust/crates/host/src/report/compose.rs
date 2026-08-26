//! Compose a report-kind dashboard's **cell grid** into the ordered, positioned pages the renderer
//! lays out (reports-as-dashboards scope, "PDF export follows the record").
//!
//! The legacy notebook was linear — one `blocks[]` entry, one full-width page — so composition was a
//! `map`. A report is a *grid*: cells carry `x`/`y`/`w`/`h`, several sit side by side, and the page
//! break falls every [`a4_rows_per_page`](lb_render::geometry::a4_rows_per_page) grid rows. So this
//! file does three things and nothing else: band the cells into pages by row, turn each cell's grid
//! rect into a page rect, and pair it with the client's PNG capture for that cell.
//!
//! **The page, not the record.** The client sends one entry per panel it actually RENDERED, each
//! carrying the grid rect it was drawn at, and this composes from that list. It used to compose from
//! the stored `cells`, which is a different list in four ways an author hits routinely — and every one
//! of them was visible in the PDF:
//!   - a **row header** is section chrome and carries no capture handle, so every sectioned report grew
//!     a full-width dashed *"not captured"* tile where its heading should be;
//!   - a panel a **filter** hid (`showWhen`), or one **parked**, or a **collapsed row's** member, was
//!     drawn as that same tile — the export announcing panels the viewer had deliberately taken away;
//!   - a **repeat clone** has a derived key (`{i}-clone-{n}`) that is in no record at all, so its
//!     capture was discarded and its SOURCE cell drew the error tile instead of the N real panels.
//! A client that sends no rendered geometry (an older one) still gets the record-driven layout, so the
//! wire change is additive rather than a flag day.
//!
//! Three rules make the output honest rather than merely plausible:
//!   - **A rendered cell with no snapshot is still placed**, as an error tile naming it. A scheduled
//!     render whose browser could not capture one panel produces a PDF with a visible hole, never a PDF
//!     that quietly omits a panel and looks complete. What changed is only WHICH cells that applies to:
//!     ones the client says were on the page, rather than every row of the record.
//!   - **Empty pages between occupied ones are kept.** If an author leaves page 2 blank, page 3 is
//!     still page 3 — the paginated document matches what they laid out.
//!   - **A panel is drawn at the size it was authored, or it moves.** Paging is by
//!     [`paginate`](lb_render::geometry::paginate), which asks whether a row band FITS rather than
//!     only where it starts, so a tall panel straddling a page boundary flows whole onto the next
//!     page instead of being clamped into the sliver left at the bottom. Squashing is reserved for a
//!     panel genuinely taller than one page, where no page could hold it.

use lb_render::geometry::{a4_rows_per_page, cell_rect_mm_on_page};
use lb_render::paginate::{page_of_row, paginate};
use lb_render::Placement;

use super::rendered::{panels_to_place, Placed, RenderedPanel};
use crate::dashboard::Cell;

/// One page's worth of composed content: its placements and the title the index shows for it.
pub struct ComposedPage {
    /// The positioned panels on this page.
    pub placements: Vec<Placement>,
    /// The index/TOC label — the first cell's title, or empty (the renderer falls back to "Page N").
    pub title: String,
}

/// Band `cells` into A4 pages and place each one, resolving its capture from `snapshots`
/// (`(cell.i, png_bytes)`).
///
/// Returns the pages in order together with the `(src, filename, bytes)` images the caller must
/// register on the assembled document — this function owns the layout, not the image plumbing.
#[allow(clippy::type_complexity)]
pub fn compose_pages(
    cells: &[Cell],
    panels: &[RenderedPanel],
) -> (Vec<ComposedPage>, Vec<(String, String, Vec<u8>)>) {
    let laid_out = panels_to_place(cells, panels);

    // Render order is reading order: top-to-bottom, then left-to-right within a row band. Neither the
    // stored `cells` array (save order, NOT visual order after a drag) nor the client's DOM order is
    // guaranteed to be that, so sort either way.
    let mut ordered: Vec<&Placed> = laid_out.iter().collect();
    ordered.sort_by_key(|c| (c.y, c.x));

    // Assign pages by whether a row band FITS, not merely where it starts: a tall panel straddling a
    // page boundary flows whole onto the next page instead of being clamped to the sliver that was
    // left. Every cell on one board row shares that row's assignment, so side-by-side tiles stay
    // together.
    let paged = paginate(
        &ordered
            .iter()
            .map(|c| (c.y, c.h))
            .collect::<Vec<(u32, u32)>>(),
    );
    let page_for = |y: u32| paged.iter().find(|r| r.y == y);

    let page_count = paged.iter().map(|r| r.page + 1).max().unwrap_or(1) as usize;
    let mut pages: Vec<ComposedPage> = (0..page_count)
        .map(|_| ComposedPage {
            placements: Vec::new(),
            title: String::new(),
        })
        .collect();

    let mut images: Vec<(String, String, Vec<u8>)> = Vec::new();

    for cell in ordered {
        // Every cell's row was fed to `paginate`, so the lookup always hits; the fixed-band rule is a
        // defensive fallback rather than an expected path.
        let (page, page_start_y) = page_for(cell.y).map_or_else(
            || {
                let per_page = a4_rows_per_page().max(1);
                (page_of_row(cell.y), cell.y - (cell.y % per_page))
            },
            |r| (r.page, r.page_start_y),
        );
        let page = page as usize;
        let src = format!("snapshot:{}", cell.id);
        let title = cell.title.clone();

        // A capture is registered only when it actually carries bytes. An EMPTY png is treated as
        // absent on purpose: the browser returns an empty string for a panel it could not rasterize,
        // and registering a zero-byte image would fail the Typst compile — taking the whole export
        // down for one uncapturable widget.
        let shot = panels.iter().find(|p| p.cell_id == cell.id);
        let note = match shot {
            Some(p) if !p.png.is_empty() => {
                images.push((src.clone(), format!("{}.png", cell.id), p.png.clone()));
                String::new()
            }
            // The client's own words when it has them ("tainted canvas", a sandboxed tile) — a reader
            // looking at the hole learns why it is there rather than only that it is.
            Some(p) if !p.reason.is_empty() => p.reason.clone(),
            _ => "not captured".to_string(),
        };

        if pages[page].title.is_empty() {
            pages[page].title.clone_from(&title);
        }
        pages[page].placements.push(Placement {
            src,
            title,
            note,
            rect: cell_rect_mm_on_page(cell.x, cell.y, cell.w, cell.h, page_start_y),
        });
    }

    (pages, images)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(i: &str, x: u32, y: u32, w: u32, h: u32, title: &str) -> Cell {
        Cell {
            i: i.into(),
            x,
            y,
            w,
            h,
            title: title.into(),
            view: "timeseries".into(),
            ..Cell::default()
        }
    }

    /// A panel the client says it RENDERED, with a capture.
    fn shot(i: &str, x: u32, y: u32, w: u32, h: u32, png: &[u8]) -> RenderedPanel {
        RenderedPanel {
            cell_id: i.into(),
            png: png.to_vec(),
            x,
            y,
            w,
            h,
            reason: String::new(),
        }
    }

    /// A panel that was on the page and could NOT be rasterised — rect, no pixels.
    fn unrasterised(i: &str, x: u32, y: u32, w: u32, h: u32, why: &str) -> RenderedPanel {
        RenderedPanel {
            cell_id: i.into(),
            reason: why.into(),
            ..shot(i, x, y, w, h, &[])
        }
    }

    /// The older wire shape: a capture with NO rendered geometry, which must still lay out from the
    /// record exactly as it always did.
    fn legacy(i: &str, png: &[u8]) -> RenderedPanel {
        RenderedPanel {
            cell_id: i.into(),
            png: png.to_vec(),
            ..RenderedPanel::default()
        }
    }

    /// Every placed panel's title on `page`, in placement order.
    fn titles(page: &ComposedPage) -> Vec<&str> {
        page.placements.iter().map(|p| p.title.as_str()).collect()
    }

    #[test]
    fn cells_are_banded_onto_pages_by_grid_row() {
        // Row 0 and its neighbour share page 0; the first row of page 1 starts it; a band beyond that
        // is page 2. Expressed in terms of `a4_rows_per_page()` rather than its current value, so the
        // print scale can move without rewriting the banding contract.
        let per = a4_rows_per_page();
        let cells = vec![
            cell("a", 0, 0, 6, 5, "A"),
            cell("b", 6, 0, 6, 5, "B"),
            cell("c", 0, per, 12, 5, "C"),
            cell("d", 0, per * 2 + 2, 12, 5, "D"),
        ];
        let (pages, _) = compose_pages(&cells, &[]);
        assert_eq!(pages.len(), 3, "three pages spanned");
        assert_eq!(pages[0].placements.len(), 2, "two panels share page 1");
        assert_eq!(pages[1].placements.len(), 1);
        assert_eq!(pages[2].placements.len(), 1);
    }

    #[test]
    fn an_empty_page_between_two_occupied_ones_is_kept() {
        // Nothing on page 1 — but page 2's content must still land on page 3 of the PDF, because
        // that is where the author put it.
        let cells = vec![
            cell("a", 0, 0, 12, 5, "A"),
            cell("b", 0, a4_rows_per_page() * 2, 12, 5, "B"),
        ];
        let (pages, _) = compose_pages(&cells, &[]);
        assert_eq!(pages.len(), 3);
        assert!(pages[1].placements.is_empty(), "the blank page survives");
        assert_eq!(pages[2].placements.len(), 1);
    }

    #[test]
    fn placement_order_is_reading_order_not_save_order() {
        // Saved bottom-first, right-first — the composed order must still be top-left first.
        let cells = vec![
            cell("bottom", 0, 6, 12, 5, "Bottom"),
            cell("right", 6, 0, 6, 5, "Right"),
            cell("left", 0, 0, 6, 5, "Left"),
        ];
        let (pages, _) = compose_pages(&cells, &[]);
        let order: Vec<&str> = pages[0]
            .placements
            .iter()
            .map(|p| p.title.as_str())
            .collect();
        assert_eq!(order, vec!["Left", "Right", "Bottom"]);
    }

    #[test]
    fn a_captured_cell_registers_its_image_and_an_uncaptured_one_is_still_placed() {
        let cells = vec![
            cell("has", 0, 0, 6, 5, "Has"),
            cell("hasnt", 6, 0, 6, 5, "Hasnt"),
        ];
        let panels = vec![
            shot("has", 0, 0, 6, 5, &[1u8, 2, 3]),
            // An EMPTY capture is the browser's "I could not rasterize this" — must read as absent.
            unrasterised("hasnt", 6, 0, 6, 5, "tainted canvas"),
        ];
        let (pages, images) = compose_pages(&cells, &panels);
        assert_eq!(images.len(), 1, "only the real capture is registered");
        assert_eq!(images[0].0, "snapshot:has");
        assert_eq!(
            pages[0].placements.len(),
            2,
            "the uncaptured panel is still placed — the hole is visible"
        );
        let missing = pages[0]
            .placements
            .iter()
            .find(|p| p.title == "Hasnt")
            .unwrap();
        // The client's own words, not the generic note — a reader learns WHY the hole is there.
        assert_eq!(missing.note, "tainted canvas");
    }

    /// THE BUG THIS FILE WAS REWRITTEN FOR. Composing from the record placed a tile for every cell the
    /// record held, whether or not it had ever been on the page — so a report grew furniture the viewer
    /// had never seen, in the shape of a dashed "not captured" box.
    #[test]
    fn a_cell_that_never_rendered_is_omitted_entirely_not_drawn_as_a_missing_panel() {
        // `hidden` is in the record and NOT in what the client rendered — a filter's `showWhen`, a park,
        // a collapsed row, or (the universal case) a row header, which carries no capture handle at all.
        let cells = vec![
            cell("shown", 0, 0, 6, 5, "Shown"),
            cell("hidden", 6, 0, 6, 5, "Hidden"),
        ];
        let (pages, _) = compose_pages(&cells, &[shot("shown", 0, 0, 6, 5, &[1u8])]);
        assert_eq!(
            titles(&pages[0]),
            vec!["Shown"],
            "only what was on the page is placed"
        );
    }

    /// The counterweight to the rule above, and the reason "was it rendered?" cannot be inferred from
    /// "did it capture?": a panel that WAS on screen and failed to rasterise must keep its hole.
    #[test]
    fn a_rendered_but_unrasterisable_panel_keeps_its_hole_while_a_hidden_one_does_not() {
        let cells = vec![
            cell("broken", 0, 0, 6, 5, "Broken"),
            cell("hidden", 6, 0, 6, 5, "Hidden"),
        ];
        let (pages, _) = compose_pages(
            &cells,
            &[unrasterised("broken", 0, 0, 6, 5, "sandboxed tile")],
        );
        assert_eq!(titles(&pages[0]), vec!["Broken"]);
        assert_eq!(pages[0].placements[0].note, "sandboxed tile");
    }

    /// A repeat clone's key (`{source}-clone-{n}`) is in NO record, so composing from the record threw
    /// its capture away and drew the SOURCE cell as a missing panel — N real tiles replaced by one hole.
    #[test]
    fn repeat_clones_are_placed_at_their_own_rects_and_carry_the_source_panel_title() {
        // One authored full-width cell; the board rendered it as three tiles side by side.
        let cells = vec![cell("meter", 0, 0, 12, 5, "Meter")];
        let panels = vec![
            shot("meter-clone-0", 0, 0, 4, 5, &[1u8]),
            shot("meter-clone-1", 4, 0, 4, 5, &[2u8]),
            shot("meter-clone-2", 8, 0, 4, 5, &[3u8]),
        ];
        let (pages, images) = compose_pages(&cells, &panels);
        assert_eq!(images.len(), 3, "every clone's capture is registered");
        assert_eq!(
            titles(&pages[0]),
            vec!["Meter", "Meter", "Meter"],
            "a clone inherits the source panel's name rather than going blank"
        );
        // Three distinct rects across the row, in reading order — not one 12-wide slot.
        let xs: Vec<f64> = pages[0].placements.iter().map(|p| p.rect.x).collect();
        assert_eq!(xs.len(), 3);
        assert!(xs[0] < xs[1] && xs[1] < xs[2], "left to right: {xs:?}");
        // …and each is a third of the board, not the source's full width.
        let (record_driven, _) = compose_pages(&cells, &[legacy("meter", &[1u8])]);
        let full = record_driven[0].placements[0].rect.w;
        assert!(
            pages[0].placements[0].rect.w < full * 0.5,
            "a clone draws at its own width, not the source's {full:.1}mm"
        );
    }

    /// The rendered rect WINS over the stored one. An author who resized a panel in a way the record has
    /// not caught up with, or a clone, must be drawn where it actually was.
    #[test]
    fn the_rect_comes_from_what_rendered_not_from_the_record() {
        let cells = vec![cell("p", 0, 0, 12, 5, "P")];
        let (record_driven, _) = compose_pages(&cells, &[legacy("p", &[1u8])]);
        let (rendered_driven, _) = compose_pages(&cells, &[shot("p", 0, 0, 6, 5, &[1u8])]);
        let full = record_driven[0].placements[0].rect.w;
        let half = rendered_driven[0].placements[0].rect.w;
        assert!(
            half < full * 0.6,
            "half-width rendered rect {half:.1}mm must not inherit the record's {full:.1}mm"
        );
    }

    /// The additive half of the wire change: a client that sends no geometry at all gets exactly the
    /// shipped layout. Without this the change would be a flag day for the headless render worker.
    #[test]
    fn a_client_that_sends_no_geometry_still_lays_out_from_the_record() {
        let cells = vec![
            cell("a", 0, 0, 6, 5, "A"),
            cell("b", 6, 0, 6, 5, "B"),
            cell("c", 0, 5, 12, 5, "C"),
        ];
        let (pages, images) = compose_pages(&cells, &[legacy("a", &[1u8]), legacy("b", &[2u8])]);
        assert_eq!(images.len(), 2);
        assert_eq!(
            titles(&pages[0]),
            vec!["A", "B", "C"],
            "every record cell is placed, uncaptured ones included — the old contract"
        );
        assert_eq!(pages[0].placements[2].note, "not captured");
    }

    /// Pagination follows the RENDERED board too. Hidden panels used to occupy row bands in the page
    /// arithmetic, so the PDF could break pages where the viewer's screen had no content at all.
    #[test]
    fn pagination_bands_only_what_was_rendered() {
        // The record spans rows 0..32 (three pages); the page only ever showed the row-0 band.
        let cells = vec![
            cell("a", 0, 0, 12, 5, "A"),
            cell("gone", 0, 16, 12, 5, "Gone"),
            cell("also-gone", 0, 32, 12, 5, "Also gone"),
        ];
        let (pages, _) = compose_pages(&cells, &[shot("a", 0, 0, 12, 5, &[1u8])]);
        assert_eq!(pages.len(), 1, "one rendered band ⇒ one page, not three");
    }

    #[test]
    fn the_page_title_is_its_first_cell_and_an_untitled_board_leaves_it_empty() {
        let (pages, _) = compose_pages(&[cell("a", 0, 0, 12, 5, "Energy use")], &[]);
        assert_eq!(pages[0].title, "Energy use");
        let (pages, _) = compose_pages(&[cell("a", 0, 0, 12, 5, "")], &[]);
        assert_eq!(pages[0].title, "", "the renderer falls back to 'Page N'");
    }

    #[test]
    fn a_report_with_no_cells_still_composes_one_page() {
        let (pages, images) = compose_pages(&[], &[]);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].placements.is_empty());
        assert!(images.is_empty());
    }
}
