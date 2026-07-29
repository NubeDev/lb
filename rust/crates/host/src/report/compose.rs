//! Compose a report-kind dashboard's **cell grid** into the ordered, positioned pages the renderer
//! lays out (reports-as-dashboards scope, "PDF export follows the record").
//!
//! The legacy notebook was linear — one `blocks[]` entry, one full-width page — so composition was a
//! `map`. A report is a *grid*: cells carry `x`/`y`/`w`/`h`, several sit side by side, and the page
//! break falls every [`a4_rows_per_page`](lb_render::geometry::a4_rows_per_page) grid rows. So this
//! file does three things and nothing else: band the cells into pages by row, turn each cell's grid
//! rect into a page rect, and pair it with the client's PNG capture for that cell.
//!
//! Two rules make the output honest rather than merely plausible:
//!   - **A cell with no snapshot is still placed**, as an error tile naming it. A scheduled render
//!     whose browser could not capture one panel produces a PDF with a visible hole, never a PDF
//!     that quietly omits a panel and looks complete.
//!   - **Empty pages between occupied ones are kept.** If an author leaves page 2 blank, page 3 is
//!     still page 3 — the paginated document matches what they laid out.

use lb_render::geometry::{cell_rect_mm, page_of_row};
use lb_render::Placement;

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
    snapshots: &[(String, Vec<u8>)],
) -> (Vec<ComposedPage>, Vec<(String, String, Vec<u8>)>) {
    // Render order is reading order: top-to-bottom, then left-to-right within a row band. The stored
    // `cells` array is in save order, which is NOT visual order after a drag.
    let mut ordered: Vec<&Cell> = cells.iter().collect();
    ordered.sort_by_key(|c| (c.y, c.x));

    let page_count = ordered
        .iter()
        .map(|c| page_of_row(c.y) + 1)
        .max()
        .unwrap_or(1) as usize;
    let mut pages: Vec<ComposedPage> = (0..page_count)
        .map(|_| ComposedPage {
            placements: Vec::new(),
            title: String::new(),
        })
        .collect();

    let mut images: Vec<(String, String, Vec<u8>)> = Vec::new();

    for cell in ordered {
        let page = page_of_row(cell.y) as usize;
        let src = format!("snapshot:{}", cell.i);
        let title = if cell.title.is_empty() {
            String::new()
        } else {
            cell.title.clone()
        };

        // A capture is registered only when it actually carries bytes. An EMPTY png is treated as
        // absent on purpose: the browser returns an empty string for a panel it could not rasterize,
        // and registering a zero-byte image would fail the Typst compile — taking the whole export
        // down for one uncapturable widget.
        let note = match snapshots.iter().find(|(k, _)| *k == cell.i) {
            Some((_, png)) if !png.is_empty() => {
                images.push((src.clone(), format!("{}.png", cell.i), png.clone()));
                String::new()
            }
            _ => "not captured".to_string(),
        };

        if pages[page].title.is_empty() {
            pages[page].title.clone_from(&title);
        }
        pages[page].placements.push(Placement {
            src,
            title,
            note,
            rect: cell_rect_mm(cell.x, cell.y, cell.w, cell.h),
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

    #[test]
    fn cells_are_banded_onto_pages_by_grid_row() {
        // 14 rows per page: row 0 and row 6 share page 0; row 14 starts page 1; row 30 is page 2.
        let cells = vec![
            cell("a", 0, 0, 6, 5, "A"),
            cell("b", 6, 0, 6, 5, "B"),
            cell("c", 0, 14, 12, 5, "C"),
            cell("d", 0, 30, 12, 5, "D"),
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
        let cells = vec![cell("a", 0, 0, 12, 5, "A"), cell("b", 0, 28, 12, 5, "B")];
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
        let snaps = vec![
            ("has".to_string(), vec![1u8, 2, 3]),
            // An EMPTY capture is the browser's "I could not rasterize this" — must read as absent.
            ("hasnt".to_string(), Vec::new()),
        ];
        let (pages, images) = compose_pages(&cells, &snaps);
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
        assert_eq!(missing.note, "not captured");
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
