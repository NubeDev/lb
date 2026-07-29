//! A4 page geometry — the ONE source the screen and the PDF share.
//!
//! A report is authored on a react-grid-layout board locked to a paper-width column, and exported to
//! A4 by the Typst template below. Those two only agree if they compute from the same numbers, so the
//! numbers live here and are mirrored **verbatim** in the shell's `ui/src/components/markdown-editor/
//! a4-sheet.ts`. [`tests::geometry_round_trips_with_the_screen_preset`] pins the derived values that
//! file asserts; if either side moves, both go red rather than silently drifting a panel off the page.
//!
//! (They HAD drifted: `a4-sheet.ts` claimed a uniform 20 mm margin while the template has always used
//! x 22 / top 24 / bottom 22. Nothing caught it because nothing laid anything out by position.)

/// ISO A4, portrait.
pub const A4_WIDTH_MM: f64 = 210.0;
/// ISO A4, portrait.
pub const A4_HEIGHT_MM: f64 = 297.0;

/// Left/right page margin the Typst template reserves (`#set page(margin: (x: 2.2cm, ..))`).
pub const A4_MARGIN_X_MM: f64 = 22.0;
/// Top page margin (larger — it carries the running header band).
pub const A4_MARGIN_TOP_MM: f64 = 24.0;
/// Bottom page margin (carries the running footer band).
pub const A4_MARGIN_BOTTOM_MM: f64 = 22.0;

/// The usable content box width — where `#place` coordinates are measured from.
pub const A4_CONTENT_W_MM: f64 = A4_WIDTH_MM - 2.0 * A4_MARGIN_X_MM;
/// The usable content box height.
pub const A4_CONTENT_H_MM: f64 = A4_HEIGHT_MM - A4_MARGIN_TOP_MM - A4_MARGIN_BOTTOM_MM;

/// CSS pixels per millimetre (the 96 dpi reference pixel). The bridge between the browser's grid,
/// which is sized in px, and the page, which is sized in mm.
pub const PX_PER_MM: f64 = 96.0 / 25.4;

/// The board's column count (`ui/src/features/dashboard/gridGeometry.ts` `GRID_COLS`).
pub const GRID_COLS: u32 = 12;
/// One grid row's height in CSS px (`gridGeometry.ts` `GRID_ROW_H`).
pub const GRID_ROW_H_PX: f64 = 56.0;
/// The gutter between cells, and the board's own padding (`gridGeometry.ts` `GRID_MARGIN`).
pub const GRID_MARGIN_PX: f64 = 10.0;

/// The paper column's pixel width — what the report builder locks the grid container to, so a cell
/// occupies the same fraction of the page on screen and in the PDF.
#[must_use]
pub fn a4_content_w_px() -> u32 {
    (A4_CONTENT_W_MM * PX_PER_MM).round() as u32
}

/// The paper column's pixel height (one page's worth of board).
#[must_use]
pub fn a4_content_h_px() -> u32 {
    (A4_CONTENT_H_MM * PX_PER_MM).round() as u32
}

/// How many whole grid rows fit on one page. The last row on a page needs no trailing gutter, hence
/// the `+ GRID_MARGIN_PX` before dividing by the row PITCH.
#[must_use]
pub fn a4_rows_per_page() -> u32 {
    ((f64::from(a4_content_h_px()) + GRID_MARGIN_PX) / (GRID_ROW_H_PX + GRID_MARGIN_PX)).floor()
        as u32
}

/// Which page a cell at grid row `y` lands on (0-based). Page breaks fall at whole-row boundaries —
/// a cell is never split across two pages, it moves wholly onto the next one.
///
/// This is the FIXED-BAND rule: rows 0..13 are page 0, 14..27 page 1, and so on. It only knows where a
/// cell *starts*, not how tall it is, so a tall cell starting near a band's end gets clamped by
/// [`cell_rect_mm`] instead of flowing. Prefer [`paginate`], which accounts for height; this remains
/// for callers that genuinely want the fixed band (and for the page a `y` belongs to in isolation).
#[must_use]
pub fn page_of_row(y: u32) -> u32 {
    y / a4_rows_per_page()
}

/// One row band's page assignment: the page it lands on, and the board row that page starts at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagedRow {
    /// The board row this entry describes.
    pub y: u32,
    /// 0-based page the row lands on.
    pub page: u32,
    /// The board row that `page` begins at — the origin [`cell_rect_mm_on_page`] measures from.
    pub page_start_y: u32,
}

/// Assign each distinct row band to a page by whether its cells actually FIT, flowing a band that
/// would overflow onto the next page instead of squashing it.
///
/// `rows` is `(y, height_in_grid_rows)` for every cell; entries sharing a `y` are one band and are
/// collapsed to the tallest, so cells laid side by side (three KPI tiles on one row) always page
/// together and never split mid-band.
///
/// Why this exists: the fixed-band rule pages on `y / rows_per_page` alone. A 7-row panel starting at
/// row 13 is "on page 0" because its top is, but only one row of space remains — so `cell_rect_mm`
/// clamped it to a sliver and the author got a thumbnail with an empty page underneath. Fitting the
/// band instead moves it whole to the next page, where it renders at its authored size. A band taller
/// than a whole page still cannot fit anywhere; it starts its own page and is clamped there, which is
/// the honest "this panel is too tall for A4" signal the clamp was written for.
///
/// Deliberate blank space is PRESERVED. The fixed band a row falls in is treated as a floor, so an
/// author who leaves a page's worth of empty rows to force a page break still gets that break; this
/// only ever moves a band *later* than the fixed rule would, never earlier. Overflow flows; layout
/// intent is not compacted away.
///
/// The returned entries are sorted by `y`, one per distinct row.
#[must_use]
pub fn paginate(rows: &[(u32, u32)]) -> Vec<PagedRow> {
    let per_page = a4_rows_per_page();
    if per_page == 0 || rows.is_empty() {
        return Vec::new();
    }

    // Collapse to one entry per row band, keeping the tallest cell on that row.
    let mut bands: Vec<(u32, u32)> = Vec::new();
    for &(y, h) in rows {
        let h = h.max(1);
        match bands.iter_mut().find(|(by, _)| *by == y) {
            Some((_, bh)) => *bh = (*bh).max(h),
            None => bands.push((y, h)),
        }
    }
    bands.sort_unstable_by_key(|(y, _)| *y);

    let mut out: Vec<PagedRow> = Vec::with_capacity(bands.len());
    let mut page = 0;
    let mut page_start_y = 0;

    for (y, h) in bands {
        // Start from where the AUTHOR put this band. Deliberate empty space is meaningful — a board
        // whose next panel sits at row 28 wants a blank page between, and compacting that away would
        // silently rewrite the author's layout. So the fixed band is a FLOOR, never a ceiling.
        let banded = y / per_page;
        if banded > page {
            page = banded;
            page_start_y = banded * per_page;
        }

        // Rows the band occupies on the current page, measured from that page's first row. `y` is
        // never below `page_start_y` because the bands are sorted and the floor only moves forward.
        let row_on_page = y - page_start_y;
        // Push to the next page when the band would run past the page's last row — unless it already
        // starts at the top, where no move can help and the clamp is the honest signal.
        if row_on_page + h > per_page && row_on_page > 0 {
            page += 1;
            page_start_y = y;
        }
        out.push(PagedRow {
            y,
            page,
            page_start_y,
        });
    }
    out
}

/// A placed rectangle on the page, in millimetres from the content box's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectMm {
    /// Offset from the content box's left edge.
    pub x: f64,
    /// Offset from the content box's top edge.
    pub y: f64,
    /// Drawn width.
    pub w: f64,
    /// Drawn height.
    pub h: f64,
}

/// Convert a grid cell (`x`/`y`/`w`/`h` in grid units, `y` absolute across the whole board) to its
/// rectangle on the page it lands on.
///
/// This is react-grid-layout's own arithmetic — column width derived from the container minus the
/// gutters, position `= colWidth * n + (n + 1) * margin` — evaluated in mm instead of px, so a panel
/// sits at the same place on paper as it did on screen. `y` is reduced modulo the page so the caller
/// passes board coordinates and gets page coordinates.
///
/// The height is CLAMPED to what remains of the content box. A cell taller than a page would
/// otherwise be `#place`d past the bottom margin, where Typst silently paints it over the footer
/// instead of flowing it — a clamp keeps it inside the page and visibly squashed, which is a fault
/// the author can see and fix.
///
/// With [`paginate`] driving the page assignment the clamp is a LAST RESORT rather than a routine
/// outcome: a band that merely straddles a page boundary now flows to the next page at full size, so
/// the only thing still clamped is a panel genuinely taller than a whole A4 content box.
#[must_use]
pub fn cell_rect_mm(x: u32, y: u32, w: u32, h: u32) -> RectMm {
    cell_rect_mm_on_page(x, y, w, h, y - (y % a4_rows_per_page()))
}

/// [`cell_rect_mm`], but measuring the vertical offset from `page_start_y` — the board row the cell's
/// page begins at, as returned by [`paginate`].
///
/// The fixed-band version derives that origin as `y - (y % rows_per_page)`, which is only correct when
/// pages break on those exact boundaries. Once a band flows early to avoid being squashed, its page
/// starts at *its* row, and the offset has to be measured from there — otherwise a panel moved to the
/// next page would be drawn at the vertical position it had on the old one.
#[must_use]
pub fn cell_rect_mm_on_page(x: u32, y: u32, w: u32, h: u32, page_start_y: u32) -> RectMm {
    let mm_per_px = A4_CONTENT_W_MM / f64::from(a4_content_w_px());
    let margin = GRID_MARGIN_PX * mm_per_px;
    let row_h = GRID_ROW_H_PX * mm_per_px;

    let cols = f64::from(GRID_COLS);
    let col_w = (A4_CONTENT_W_MM - margin * (cols + 1.0)) / cols;

    let row_on_page = f64::from(y.saturating_sub(page_start_y));
    let x = f64::from(x);
    let w = f64::from(w.max(1));
    let h = f64::from(h.max(1));

    let top = (row_h + margin) * row_on_page + margin;
    let height = row_h * h + margin * (h - 1.0);
    RectMm {
        x: col_w * x + (x + 1.0) * margin,
        y: top,
        w: col_w * w + margin * (w - 1.0),
        h: height.min(A4_CONTENT_H_MM - top),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The screen↔print contract. These exact numbers are asserted on the other side by
    /// `ui/src/components/markdown-editor/a4-sheet.test.ts`; changing one without the other is the
    /// drift this test exists to make impossible.
    #[test]
    fn geometry_round_trips_with_the_screen_preset() {
        assert_eq!(A4_CONTENT_W_MM, 166.0);
        assert_eq!(A4_CONTENT_H_MM, 251.0);
        assert_eq!(a4_content_w_px(), 627);
        assert_eq!(a4_content_h_px(), 949);
        assert_eq!(a4_rows_per_page(), 14);
    }

    #[test]
    fn page_breaks_fall_every_whole_page_of_rows() {
        assert_eq!(page_of_row(0), 0);
        assert_eq!(page_of_row(13), 0);
        assert_eq!(page_of_row(14), 1);
        assert_eq!(page_of_row(27), 1);
        assert_eq!(page_of_row(28), 2);
    }

    /// The bug this whole change exists for, in the shape it actually shipped in: the demo energy
    /// report's water chart (7 rows tall, starting at row 13). The fixed band called row 13 "page 0"
    /// because its TOP fits, leaving ~1 row of space, so the clamp crushed a 119.7 mm panel to 21.2 mm
    /// — a thumbnail — while the page below it sat empty.
    #[test]
    fn a_tall_band_that_would_be_squashed_flows_to_the_next_page_at_full_height() {
        // hdr, the three KPI tiles (one band), the trend chart, the water chart, the table.
        let rows = [(0, 4), (4, 4), (4, 4), (4, 4), (8, 5), (13, 7), (20, 7)];
        let paged = paginate(&rows);
        let page_of = |y: u32| paged.iter().find(|r| r.y == y).unwrap();

        assert_eq!(page_of(0).page, 0);
        assert_eq!(page_of(4).page, 0);
        assert_eq!(page_of(8).page, 0);
        // The fix: row 13 no longer clings to page 0 just because its top fits there.
        assert_eq!(page_of(13).page, 1, "the water chart must flow, not squash");
        assert_eq!(
            page_of(20).page,
            1,
            "the table follows it onto the same page"
        );

        // And on its new page it is measured from ITS OWN first row, so it draws at full height
        // rather than being clamped by an offset inherited from the page it left.
        let start = page_of(13).page_start_y;
        assert_eq!(start, 13);
        let r = cell_rect_mm_on_page(0, 13, 12, 7, start);
        let unclamped = cell_rect_mm_on_page(0, 0, 12, 7, 0);
        assert!(
            (r.h - unclamped.h).abs() < 0.01,
            "expected full height {:.1}mm, drew {:.1}mm",
            unclamped.h,
            r.h
        );
    }

    #[test]
    fn cells_sharing_a_row_always_page_together() {
        // Three KPI tiles side by side, plus a tall neighbour on the same row. The band pages as one,
        // so a row can never be torn in half across a page boundary.
        let paged = paginate(&[(10, 2), (10, 2), (10, 8)]);
        assert_eq!(paged.len(), 1, "one entry per row band, not per cell");
        assert_eq!(paged[0].y, 10);
    }

    #[test]
    fn a_band_taller_than_a_whole_page_still_gets_its_own_page_and_is_clamped() {
        // 20 rows cannot fit on a 14-row page anywhere. It must not loop forever looking for room; it
        // starts a page and the clamp remains the honest "too tall for A4" signal.
        let paged = paginate(&[(0, 4), (4, 20)]);
        let tall = paged.iter().find(|r| r.y == 4).unwrap();
        assert_eq!(tall.page, 1);
        assert_eq!(tall.page_start_y, 4);
        let r = cell_rect_mm_on_page(0, 4, 12, 20, tall.page_start_y);
        assert!(r.h <= A4_CONTENT_H_MM, "must stay inside the content box");
    }

    #[test]
    fn a_board_that_already_fits_is_paginated_exactly_as_before() {
        // No regression for the common case: nothing overflows, so nothing moves.
        let rows = [(0, 4), (4, 4), (8, 4)];
        for r in paginate(&rows) {
            assert_eq!(r.page, 0);
            assert_eq!(r.page_start_y, 0);
            // Same origin as the fixed-band rule ⇒ identical rectangles.
            assert_eq!(
                cell_rect_mm_on_page(0, r.y, 12, 4, r.page_start_y),
                cell_rect_mm(0, r.y, 12, 4)
            );
        }
    }

    #[test]
    fn paginate_is_empty_for_no_rows() {
        assert!(paginate(&[]).is_empty());
    }

    /// The counterweight to the flow rule: an author who leaves a whole page of empty rows means it.
    /// Flowing must never COMPACT — it only ever pushes a band later than the fixed rule would.
    #[test]
    fn deliberate_blank_pages_are_preserved_not_compacted() {
        // Nothing occupies page 1; the row-28 band belongs on page 2 and must stay there.
        let paged = paginate(&[(0, 5), (28, 5)]);
        assert_eq!(paged.iter().find(|r| r.y == 0).unwrap().page, 0);
        let late = paged.iter().find(|r| r.y == 28).unwrap();
        assert_eq!(late.page, 2, "the author's blank page survives");
        assert_eq!(late.page_start_y, 28);
    }

    #[test]
    fn a_full_width_cell_spans_the_content_box_between_the_gutters() {
        let r = cell_rect_mm(0, 0, 12, 4);
        let margin = GRID_MARGIN_PX * (A4_CONTENT_W_MM / f64::from(a4_content_w_px()));
        // Left gutter in, right gutter out: the drawn width is the content box minus both gutters.
        assert!((r.x - margin).abs() < 1e-9, "x = one gutter, got {}", r.x);
        assert!(
            (r.x + r.w - (A4_CONTENT_W_MM - margin)).abs() < 1e-9,
            "right edge = content width minus one gutter, got {}",
            r.x + r.w
        );
    }

    #[test]
    fn two_half_width_cells_tile_the_row_without_overlapping() {
        let left = cell_rect_mm(0, 0, 6, 4);
        let right = cell_rect_mm(6, 0, 6, 4);
        assert!((left.w - right.w).abs() < 1e-9, "halves are equal width");
        assert!(
            left.x + left.w <= right.x + 1e-9,
            "left {:?} must end before right {:?} begins",
            left,
            right
        );
        assert_eq!(left.y, right.y, "same row, same top");
    }

    #[test]
    fn a_cell_on_the_second_page_is_measured_from_that_page_top() {
        // Row 14 is the first row of page 1 — it must sit at the same offset row 0 does on page 0.
        assert_eq!(cell_rect_mm(0, 0, 6, 4).y, cell_rect_mm(0, 14, 6, 4).y);
        assert_eq!(page_of_row(14), 1);
    }

    #[test]
    fn an_over_tall_cell_is_clamped_inside_the_content_box() {
        // 40 rows is far more than a page holds; the rect must still end inside the page.
        let r = cell_rect_mm(0, 0, 12, 40);
        assert!(
            r.y + r.h <= A4_CONTENT_H_MM + 1e-9,
            "clamped rect {r:?} must not spill past {A4_CONTENT_H_MM}mm"
        );
    }
}
