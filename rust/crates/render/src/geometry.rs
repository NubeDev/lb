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

/// How much WIDER than true A4 the report board lays out, and therefore how far the PDF scales the
/// whole thing back down. `1.0` is true size; `2.0` lays the board out at twice the printable width and
/// reduces it by half onto the page. Mirrored verbatim in `a4-sheet.ts` as `REPORT_PRINT_SCALE`.
///
/// WHY IT IS NOT 1. At true size the board's column is 627px — roughly half a desktop's reading width —
/// and the panels do not merely look smaller there, they RE-FLOW: a chart given 300px instead of 550px
/// drops axis ticks, wraps its legend and truncates labels. The exported report was therefore not a
/// smaller copy of the page but a differently-laid-out one, which is what "the PDF looks squeezed"
/// meant. Laying out wide and reducing uniformly fixes it by construction — every panel keeps the
/// proportions and the tick density it had on screen, and the page holds a reduced photograph of it.
///
/// The cost is real and unavoidable: content prints at `1/SCALE` of physical size, so a 12px axis label
/// lands near 4.5pt at `2.0`. That trade is what this constant IS, which is why it is one named number
/// rather than arithmetic spread across the file.
pub const REPORT_PRINT_SCALE: f64 = 2.0;

/// The A4 printable width in CSS px — TRUE size, the 96 dpi reference. This is the page, not the
/// layout: see [`report_layout_w_px`] for the width the board is actually laid out at.
#[must_use]
pub fn a4_content_w_px() -> u32 {
    (A4_CONTENT_W_MM * PX_PER_MM).round() as u32
}

/// The paper column's pixel width — what the report builder locks the grid container to, so a cell
/// occupies the same fraction of the page on screen and in the PDF. `REPORT_PRINT_SCALE ×` the true
/// printable width; the reduction back onto the page happens in [`cell_rect_mm_on_page`], which derives
/// its millimetres-per-pixel from this rather than from the true-size width.
#[must_use]
pub fn report_layout_w_px() -> u32 {
    (f64::from(a4_content_w_px()) * REPORT_PRINT_SCALE).round() as u32
}

/// One A4 page's height in the LAYOUT's pixels — the page box scaled by the same factor as the width,
/// so a page stays the same shape whatever the scale.
#[must_use]
pub fn report_layout_page_h_px() -> u32 {
    (f64::from(a4_content_h_px()) * REPORT_PRINT_SCALE).round() as u32
}

/// The paper column's pixel height (one page's worth of board).
#[must_use]
pub fn a4_content_h_px() -> u32 {
    (A4_CONTENT_H_MM * PX_PER_MM).round() as u32
}

/// How many whole grid rows fit on one page. The last row on a page needs no trailing gutter, hence
/// the `+ GRID_MARGIN_PX` before dividing by the row PITCH.
///
/// Measured against the SCALED page height, because the rows are laid out in scaled pixels too. At
/// scale 2 that is 28 rows to a page rather than 14 — the same paper holding twice as much board, which
/// is exactly what scaling down buys.
#[must_use]
pub fn a4_rows_per_page() -> u32 {
    ((f64::from(report_layout_page_h_px()) + GRID_MARGIN_PX) / (GRID_ROW_H_PX + GRID_MARGIN_PX))
        .floor() as u32
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
    // Millimetres per LAYOUT pixel — the uniform reduction. Deriving it from `report_layout_w_px`
    // rather than the true-size width is the whole scale-down: the board's own px arithmetic below is
    // unchanged, so every rect comes out at exactly `1/REPORT_PRINT_SCALE` of true size and a panel's
    // aspect ratio is preserved to the last decimal.
    let mm_per_px = A4_CONTENT_W_MM / f64::from(report_layout_w_px());
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
    use crate::paginate::{page_of_row, paginate};

    /// The screen↔print contract. These exact numbers are asserted on the other side by
    /// `ui/src/components/markdown-editor/a4-sheet.test.ts`; changing one without the other is the
    /// drift this test exists to make impossible.
    #[test]
    fn geometry_round_trips_with_the_screen_preset() {
        // The PAGE — true size, unchanged, and what the Typst template's margins produce.
        assert_eq!(A4_CONTENT_W_MM, 166.0);
        assert_eq!(A4_CONTENT_H_MM, 251.0);
        assert_eq!(a4_content_w_px(), 627);
        assert_eq!(a4_content_h_px(), 949);
        // The LAYOUT — what the board is actually laid out at, and reduced from.
        assert_eq!(REPORT_PRINT_SCALE, 2.0);
        assert_eq!(report_layout_w_px(), 1254);
        assert_eq!(report_layout_page_h_px(), 1898);
        assert_eq!(a4_rows_per_page(), 28);
    }

    /// THE PROPERTY THE SCALE-DOWN IS FOR. A panel's printed rectangle must have the SAME aspect ratio
    /// as the box it was rendered in, at every size — otherwise `fit: "contain"` letterboxes the
    /// capture inside its slot and the panel reads as shrunken with white space around it.
    ///
    /// This is checked against react-grid-layout's own arithmetic at the layout width, which is the
    /// thing the browser actually did, not against a restatement of the formula below.
    #[test]
    fn a_printed_rect_has_the_same_aspect_as_the_box_the_browser_rendered() {
        let w_px = f64::from(report_layout_w_px());
        // RGL: colWidth = (containerWidth − margin×(cols−1) − padding×2) / cols.
        let col_w_px =
            (w_px - GRID_MARGIN_PX * (f64::from(GRID_COLS) - 1.0) - GRID_MARGIN_PX * 2.0)
                / f64::from(GRID_COLS);

        for (w, h) in [(1_u32, 1_u32), (4, 3), (6, 5), (12, 2), (3, 8), (12, 20)] {
            let px_w = col_w_px * f64::from(w) + GRID_MARGIN_PX * f64::from(w - 1);
            let px_h = GRID_ROW_H_PX * f64::from(h) + GRID_MARGIN_PX * f64::from(h - 1);
            let r = cell_rect_mm_on_page(0, 0, w, h, 0);
            let drift = ((r.w / r.h) - (px_w / px_h)).abs() / (px_w / px_h);
            assert!(
                drift < 0.001,
                "{w}x{h}: rendered {px_w:.1}x{px_h:.1}px (aspect {:.4}) vs printed {:.1}x{:.1}mm (aspect {:.4})",
                px_w / px_h,
                r.w,
                r.h,
                r.w / r.h,
            );
        }
    }

    /// …and the reduction is UNIFORM: every rect is exactly `1/REPORT_PRINT_SCALE` of the true-size
    /// millimetres it would have had. Aspect alone would still allow a per-panel scale drift.
    #[test]
    fn the_whole_board_reduces_by_exactly_the_print_scale() {
        let mm_per_layout_px = A4_CONTENT_W_MM / f64::from(report_layout_w_px());
        let mm_per_true_px = A4_CONTENT_W_MM / f64::from(a4_content_w_px());
        assert!(
            (mm_per_true_px / mm_per_layout_px - REPORT_PRINT_SCALE).abs() < 1e-9,
            "the reduction must be exactly the scale, got {}",
            mm_per_true_px / mm_per_layout_px
        );
    }

    /// The bug this whole change exists for, in the shape it actually shipped in: the demo energy
    /// report's water chart (7 rows tall, starting at row 13). The fixed band called row 13 "page 0"
    /// because its TOP fits, leaving ~1 row of space, so the clamp crushed a 119.7 mm panel to 21.2 mm
    /// — a thumbnail — while the page below it sat empty.
    #[test]
    fn a_tall_band_that_would_be_squashed_flows_to_the_next_page_at_full_height() {
        // hdr, the three KPI tiles (one band), the trend chart, the water chart, the table — scaled to
        // the current rows-per-page so the shape of the bug is preserved rather than its old numbers:
        // the water chart's band starts just inside the page and is taller than the room left.
        let per = a4_rows_per_page();
        let (a, b, c, d) = (per / 2, per - 1, 7, per * 3 / 2);
        let rows = [(0, 4), (4, 4), (4, 4), (4, 4), (a, 5), (b, c), (d, c)];
        let paged = paginate(&rows);
        let page_of = |y: u32| paged.iter().find(|r| r.y == y).unwrap();

        assert_eq!(page_of(0).page, 0);
        assert_eq!(page_of(4).page, 0);
        assert_eq!(page_of(a).page, 0);
        // The fix: the band no longer clings to page 0 just because its top fits there.
        assert_eq!(page_of(b).page, 1, "the water chart must flow, not squash");
        assert_eq!(
            page_of(d).page,
            1,
            "the table follows it onto the same page"
        );

        // And on its new page it is measured from ITS OWN first row, so it draws at full height
        // rather than being clamped by an offset inherited from the page it left.
        let start = page_of(b).page_start_y;
        assert_eq!(start, b);
        let r = cell_rect_mm_on_page(0, b, 12, c, start);
        let unclamped = cell_rect_mm_on_page(0, 0, 12, c, 0);
        assert!(
            (r.h - unclamped.h).abs() < 0.01,
            "expected full height {:.1}mm, drew {:.1}mm",
            unclamped.h,
            r.h
        );
    }

    #[test]
    fn a_band_taller_than_a_whole_page_still_gets_its_own_page_and_is_clamped() {
        // A band taller than a whole page cannot fit anywhere. It must not loop forever looking for
        // room; it starts a page and the clamp remains the honest "too tall for A4" signal.
        let too_tall = a4_rows_per_page() + 6;
        let paged = paginate(&[(0, 4), (4, too_tall)]);
        let tall = paged.iter().find(|r| r.y == 4).unwrap();
        assert_eq!(tall.page, 1);
        assert_eq!(tall.page_start_y, 4);
        let r = cell_rect_mm_on_page(0, 4, 12, too_tall, tall.page_start_y);
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

    /// The counterweight to the flow rule: an author who leaves a whole page of empty rows means it.
    /// Flowing must never COMPACT — it only ever pushes a band later than the fixed rule would.

    #[test]
    fn a_full_width_cell_spans_the_content_box_between_the_gutters() {
        let r = cell_rect_mm(0, 0, 12, 4);
        let margin = GRID_MARGIN_PX * (A4_CONTENT_W_MM / f64::from(report_layout_w_px()));
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
        // The first row of page 1 must sit at the same offset row 0 does on page 0.
        let per = a4_rows_per_page();
        assert_eq!(cell_rect_mm(0, 0, 6, 4).y, cell_rect_mm(0, per, 6, 4).y);
        assert_eq!(page_of_row(per), 1);
    }

    #[test]
    fn an_over_tall_cell_is_clamped_inside_the_content_box() {
        // Far more than a page holds; the rect must still end inside the page.
        let r = cell_rect_mm(0, 0, 12, a4_rows_per_page() * 3);
        assert!(
            r.y + r.h <= A4_CONTENT_H_MM + 1e-9,
            "clamped rect {r:?} must not spill past {A4_CONTENT_H_MM}mm"
        );
    }
}
