//! WHERE on its page a grid cell sits — the placement half of the report geometry.
//!
//! Split from [`super::geometry`], which owns the page BOX, and from [`super::paginate`], which owns
//! which PAGE a band lands on. Placement is millimetres; paging is bands and fit; the box is the paper.
//! They share only [`PageGeometry`], so they read better apart than as one 500-line file.
//!
//! The arithmetic here is react-grid-layout's own, evaluated in mm instead of px — that is the whole
//! trick that makes a panel sit in the same place on paper as it did on screen.

use crate::geometry::{GRID_COLS, GRID_MARGIN_PX, GRID_ROW_H_PX, PageGeometry};

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

impl PageGeometry {
    /// Convert a grid cell (`x`/`y`/`w`/`h` in grid units, `y` absolute across the whole board) to its
    /// rectangle on the page it lands on, under the FIXED-BAND page origin.
    ///
    /// Prefer [`Self::cell_rect_mm_on_page`] with the origin [`crate::paginate::paginate`] returned:
    /// this one derives the origin as `y - (y % rows_per_page)`, which is only correct while pages
    /// break on those exact boundaries.
    #[must_use]
    pub fn cell_rect_mm(&self, x: u32, y: u32, w: u32, h: u32) -> RectMm {
        let per = self.rows_per_page().max(1);
        self.cell_rect_mm_on_page(x, y, w, h, y - (y % per))
    }

    /// [`Self::cell_rect_mm`], but measuring the vertical offset from `page_start_y` — the board row
    /// the cell's page begins at, as returned by [`crate::paginate::paginate`].
    ///
    /// Once a band flows early to avoid being squashed, its page starts at *its* row, and the offset
    /// has to be measured from there — otherwise a panel moved to the next page would be drawn at the
    /// vertical position it had on the old one.
    ///
    /// The height is CLAMPED to what remains of the content box. A cell taller than a page would
    /// otherwise be `#place`d past the bottom margin, where Typst silently paints it over the footer
    /// instead of flowing it — a clamp keeps it inside the page and visibly squashed, which is a fault
    /// the author can see and fix. With `paginate` driving the page assignment that clamp is a LAST
    /// RESORT rather than a routine outcome: the only thing still clamped is a panel genuinely taller
    /// than a whole content box.
    #[must_use]
    pub fn cell_rect_mm_on_page(
        &self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        page_start_y: u32,
    ) -> RectMm {
        let content_w = self.content_w_mm();
        let content_h = self.content_h_mm();

        // Millimetres per LAYOUT pixel — the uniform reduction. Deriving it from `layout_w_px` rather
        // than the true-size width is the whole scale-down: the board's own px arithmetic below is
        // unchanged, so every rect comes out at exactly `1/scale` of true size and a panel's aspect
        // ratio is preserved to the last decimal.
        let mm_per_px = content_w / f64::from(self.layout_w_px());
        let margin = GRID_MARGIN_PX * mm_per_px;
        let row_h = GRID_ROW_H_PX * mm_per_px;

        let cols = f64::from(GRID_COLS);
        let col_w = (content_w - margin * (cols + 1.0)) / cols;

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
            h: height.min(content_h - top),
        }
    }
}

/// [`PageGeometry::cell_rect_mm`] on the A4-portrait default. Shorthand for the many callers that
/// have no options in hand.
#[must_use]
pub fn cell_rect_mm(x: u32, y: u32, w: u32, h: u32) -> RectMm {
    PageGeometry::a4_portrait().cell_rect_mm(x, y, w, h)
}

/// [`PageGeometry::cell_rect_mm_on_page`] on the A4-portrait default.
#[must_use]
pub fn cell_rect_mm_on_page(x: u32, y: u32, w: u32, h: u32, page_start_y: u32) -> RectMm {
    PageGeometry::a4_portrait().cell_rect_mm_on_page(x, y, w, h, page_start_y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{A4_CONTENT_H_MM, A4_CONTENT_W_MM};
    use crate::paginate::{page_of_row, paginate};

    /// THE PROPERTY THE SCALE-DOWN IS FOR. A panel's printed rectangle must have the SAME aspect ratio
    /// as the box it was rendered in, at every size — otherwise `fit: "contain"` letterboxes the
    /// capture inside its slot and the panel reads as shrunken with white space around it.
    ///
    /// This is checked against react-grid-layout's own arithmetic at the layout width, which is the
    /// thing the browser actually did, not against a restatement of the formula below — and now for
    /// EVERY geometry, because the scale and the paper are parameters.
    #[test]
    fn a_printed_rect_has_the_same_aspect_as_the_box_the_browser_rendered() {
        for g in [
            PageGeometry::a4_portrait(),
            PageGeometry::a4_portrait().landscape(),
            PageGeometry {
                scale: 1.0,
                ..PageGeometry::a4_portrait()
            },
        ] {
            let w_px = f64::from(g.layout_w_px());
            // RGL: colWidth = (containerWidth − margin×(cols−1) − padding×2) / cols.
            let col_w_px =
                (w_px - GRID_MARGIN_PX * (f64::from(GRID_COLS) - 1.0) - GRID_MARGIN_PX * 2.0)
                    / f64::from(GRID_COLS);

            for (w, h) in [(1_u32, 1_u32), (4, 3), (6, 5), (12, 2), (3, 8)] {
                let px_w = col_w_px * f64::from(w) + GRID_MARGIN_PX * f64::from(w - 1);
                let px_h = GRID_ROW_H_PX * f64::from(h) + GRID_MARGIN_PX * f64::from(h - 1);
                let r = g.cell_rect_mm_on_page(0, 0, w, h, 0);
                let drift = ((r.w / r.h) - (px_w / px_h)).abs() / (px_w / px_h);
                assert!(
                    drift < 0.001,
                    "{w}x{h} on {g:?}: rendered aspect {:.4} vs printed {:.4}",
                    px_w / px_h,
                    r.w / r.h,
                );
            }
        }
    }

    #[test]
    fn a_full_width_cell_spans_the_content_box_between_the_gutters() {
        let r = cell_rect_mm(0, 0, 12, 4);
        let margin = GRID_MARGIN_PX * (A4_CONTENT_W_MM / f64::from(report_layout_w_px_for_test()));
        // Left gutter in, right gutter out: the drawn width is the content box minus both gutters.
        assert!((r.x - margin).abs() < 1e-9, "x = one gutter, got {}", r.x);
        assert!(
            (r.x + r.w - (A4_CONTENT_W_MM - margin)).abs() < 1e-9,
            "right edge = content width minus one gutter, got {}",
            r.x + r.w
        );
    }

    fn report_layout_w_px_for_test() -> u32 {
        PageGeometry::a4_portrait().layout_w_px()
    }

    #[test]
    fn two_half_width_cells_tile_the_row_without_overlapping() {
        let left = cell_rect_mm(0, 0, 6, 4);
        let right = cell_rect_mm(6, 0, 6, 4);
        assert!((left.w - right.w).abs() < 1e-9, "halves are equal width");
        assert!(
            left.x + left.w <= right.x + 1e-9,
            "left {left:?} must end before right {right:?} begins",
        );
        assert_eq!(left.y, right.y, "same row, same top");
    }

    #[test]
    fn a_cell_on_the_second_page_is_measured_from_that_page_top() {
        // The first row of page 1 must sit at the same offset row 0 does on page 0.
        let per = PageGeometry::a4_portrait().rows_per_page();
        assert_eq!(cell_rect_mm(0, 0, 6, 4).y, cell_rect_mm(0, per, 6, 4).y);
        assert_eq!(page_of_row(per), 1);
    }

    #[test]
    fn an_over_tall_cell_is_clamped_inside_the_content_box() {
        // Far more than a page holds; the rect must still end inside the page, on any paper.
        for g in [
            PageGeometry::a4_portrait(),
            PageGeometry::a4_portrait().landscape(),
        ] {
            let r = g.cell_rect_mm(0, 0, 12, g.rows_per_page() * 3);
            assert!(
                r.y + r.h <= g.content_h_mm() + 1e-9,
                "clamped rect {r:?} must not spill past {}mm",
                g.content_h_mm()
            );
        }
        assert!(cell_rect_mm(0, 0, 12, 999).h <= A4_CONTENT_H_MM);
    }

    /// The bug the fit rule exists for, in the shape it actually shipped in: the demo energy report's
    /// water chart (7 rows tall, starting just inside the page). The fixed band called its row "page 0"
    /// because its TOP fits, leaving ~1 row of space, so the clamp crushed a 119.7 mm panel to 21.2 mm
    /// — a thumbnail — while the page below it sat empty.
    ///
    /// Lived in `geometry.rs` until placement moved here; it asserts a PLACEMENT under a pagination,
    /// so it belongs beside the placement maths.
    #[test]
    fn a_tall_band_that_would_be_squashed_flows_to_the_next_page_at_full_height() {
        let per = PageGeometry::a4_portrait().rows_per_page();
        let (a, b, c, d) = (per / 2, per - 1, 7, per * 3 / 2);
        let rows = [(0, 4), (4, 4), (4, 4), (4, 4), (a, 5), (b, c), (d, c)];
        let paged = paginate(&rows);
        let page_of = |y: u32| *paged.iter().find(|r| r.y == y).unwrap();

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
        let too_tall = PageGeometry::a4_portrait().rows_per_page() + 6;
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
}
