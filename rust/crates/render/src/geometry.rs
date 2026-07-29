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
#[must_use]
pub fn page_of_row(y: u32) -> u32 {
    y / a4_rows_per_page()
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
#[must_use]
pub fn cell_rect_mm(x: u32, y: u32, w: u32, h: u32) -> RectMm {
    let mm_per_px = A4_CONTENT_W_MM / f64::from(a4_content_w_px());
    let margin = GRID_MARGIN_PX * mm_per_px;
    let row_h = GRID_ROW_H_PX * mm_per_px;

    let cols = f64::from(GRID_COLS);
    let col_w = (A4_CONTENT_W_MM - margin * (cols + 1.0)) / cols;

    let row_on_page = f64::from(y % a4_rows_per_page());
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
