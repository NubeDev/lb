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
//!     [`paginate_with`](lb_render::paginate::paginate_with), which honours the author's explicit
//!     page-break markers and otherwise asks whether a row band FITS rather than
//!     only where it starts, so a tall panel straddling a page boundary flows whole onto the next
//!     page instead of being clamped into the sliver left at the bottom. Squashing is reserved for a
//!     panel genuinely taller than one page, where no page could hold it.

use lb_render::geometry::PageGeometry;
use lb_render::paginate::{page_of_row, paginate_with, Band};
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
    geo: &PageGeometry,
) -> (Vec<ComposedPage>, Vec<(String, String, Vec<u8>)>) {
    let laid_out = panels_to_place(cells, panels);

    // Render order is reading order: top-to-bottom, then left-to-right within a row band. Neither the
    // stored `cells` array (save order, NOT visual order after a drag) nor the client's DOM order is
    // guaranteed to be that, so sort either way.
    let mut ordered: Vec<&Placed> = laid_out.iter().collect();
    ordered.sort_by_key(|c| (c.y, c.x));

    // Assign pages by the author's markers first and by whether a row band FITS second: a marked band
    // starts a new page, and a tall panel straddling a page boundary flows whole onto the next one
    // instead of being clamped to the sliver that was left. Every cell on one board row shares that
    // row's assignment, so side-by-side tiles stay together.
    let paged = paginate_with(
        geo,
        &ordered
            .iter()
            .map(|c| Band {
                y: c.y,
                h: c.h,
                break_before: c.break_before,
            })
            .collect::<Vec<Band>>(),
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
                let per_page = geo.rows_per_page().max(1);
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
            rect: geo.cell_rect_mm_on_page(cell.x, cell.y, cell.w, cell.h, page_start_y),
        });
    }

    (pages, images)
}

// The tests live next door (`compose_tests.rs`, included as this module's `tests`) rather than at the
// foot of this file: composition needs a lot of fixture to say anything true, and carrying it here put
// the file over the FILE-LAYOUT 400-line ceiling. Same module, same privacy, one responsibility each.
#[cfg(test)]
#[path = "compose_tests.rs"]
mod tests;
