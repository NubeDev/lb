//! Page geometry — the ONE source the screen and the PDF share.
//!
//! A report is authored on a react-grid-layout board locked to a paper-width column, and exported by
//! the Typst template. Those two only agree if they compute from the same numbers, so the numbers live
//! here and the A4 ones are mirrored **verbatim** in the shell's
//! `ui/src/components/markdown-editor/a4-sheet.ts`. [`tests::geometry_round_trips_with_the_screen_preset`]
//! pins the derived values that file asserts; if either side moves, both go red rather than silently
//! drifting a panel off the page.
//!
//! (They HAD drifted: `a4-sheet.ts` claimed a uniform 20 mm margin while the template has always used
//! x 22 / top 24 / bottom 22. Nothing caught it because nothing laid anything out by position.)
//!
//! **The page is a VALUE now, not six constants.** [`PageGeometry`] carries the page box and the print
//! scale, and everything derived — the content box, the pixel widths, the rows that fit on a page — is
//! a method on it. A4 portrait is [`PageGeometry::a4_portrait`] and remains the default to the
//! millimetre; the `A4_*` consts below are what defines it and what the screen mirrors. Nothing
//! computes A4 from anything else, so "the default changed by accident" is not a reachable state.
//!
//! Where a cell LANDS on the page is the other half of the question and lives in
//! [`super::cell_rect`]; which PAGE a band lands on is [`super::paginate`]. Three questions, three
//! files, one shared page box.

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

/// The usable A4-portrait content box width — where `#place` coordinates are measured from.
pub const A4_CONTENT_W_MM: f64 = A4_WIDTH_MM - 2.0 * A4_MARGIN_X_MM;
/// The usable A4-portrait content box height.
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

/// How much WIDER than true size the report board lays out, and therefore how far the PDF scales the
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
///
/// It is the DEFAULT scale, not the only one: [`PageGeometry::scale`] carries the value in force, and a
/// wider paper reasonably wants a different one.
pub const REPORT_PRINT_SCALE: f64 = 2.0;

/// One page's box, in millimetres, plus the print scale the board is laid out at.
///
/// Everything the placement and pagination maths needs is derived from these six numbers, so a caller
/// that wants a different paper changes the value rather than the code. `Default` is A4 portrait.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageGeometry {
    /// Paper width.
    pub w_mm: f64,
    /// Paper height.
    pub h_mm: f64,
    /// Left/right margin.
    pub margin_x_mm: f64,
    /// Top margin (carries the running header band).
    pub margin_top_mm: f64,
    /// Bottom margin (carries the running footer band).
    pub margin_bottom_mm: f64,
    /// The layout-to-paper reduction; see [`REPORT_PRINT_SCALE`].
    pub scale: f64,
}

impl Default for PageGeometry {
    fn default() -> Self {
        Self::a4_portrait()
    }
}

impl PageGeometry {
    /// The shipped default, to the millimetre. Every value comes from the `A4_*` consts above, which
    /// are the same numbers `a4-sheet.ts` mirrors — so the default cannot drift from the screen
    /// without the round-trip test noticing.
    #[must_use]
    pub const fn a4_portrait() -> Self {
        Self {
            w_mm: A4_WIDTH_MM,
            h_mm: A4_HEIGHT_MM,
            margin_x_mm: A4_MARGIN_X_MM,
            margin_top_mm: A4_MARGIN_TOP_MM,
            margin_bottom_mm: A4_MARGIN_BOTTOM_MM,
            scale: REPORT_PRINT_SCALE,
        }
    }

    /// The same page turned on its side. Margins do NOT rotate with it — the top margin carries the
    /// header band and the sides carry nothing, so swapping them would move the header, not the paper.
    #[must_use]
    pub const fn landscape(self) -> Self {
        Self {
            w_mm: self.h_mm,
            h_mm: self.w_mm,
            ..self
        }
    }

    /// The usable content box width — where `#place` coordinates are measured from.
    #[must_use]
    pub fn content_w_mm(&self) -> f64 {
        self.w_mm - 2.0 * self.margin_x_mm
    }

    /// The usable content box height.
    #[must_use]
    pub fn content_h_mm(&self) -> f64 {
        self.h_mm - self.margin_top_mm - self.margin_bottom_mm
    }

    /// The printable width in CSS px — TRUE size, the 96 dpi reference. This is the page, not the
    /// layout: see [`Self::layout_w_px`] for the width the board is actually laid out at.
    #[must_use]
    pub fn content_w_px(&self) -> u32 {
        (self.content_w_mm() * PX_PER_MM).round() as u32
    }

    /// The printable height in CSS px (one page's worth of board, true size).
    #[must_use]
    pub fn content_h_px(&self) -> u32 {
        (self.content_h_mm() * PX_PER_MM).round() as u32
    }

    /// The paper column's pixel width — what the report builder locks the grid container to, so a cell
    /// occupies the same fraction of the page on screen and in the PDF. `scale ×` the true printable
    /// width; the reduction back onto the page happens in [`Self::cell_rect_mm_on_page`], which derives
    /// its millimetres-per-pixel from this rather than from the true-size width.
    #[must_use]
    pub fn layout_w_px(&self) -> u32 {
        (f64::from(self.content_w_px()) * self.scale).round() as u32
    }

    /// One page's height in the LAYOUT's pixels — the page box scaled by the same factor as the width,
    /// so a page stays the same shape whatever the scale.
    #[must_use]
    pub fn layout_page_h_px(&self) -> u32 {
        (f64::from(self.content_h_px()) * self.scale).round() as u32
    }

    /// How many whole grid rows fit on one page. The last row on a page needs no trailing gutter, hence
    /// the `+ GRID_MARGIN_PX` before dividing by the row PITCH.
    ///
    /// Measured against the SCALED page height, because the rows are laid out in scaled pixels too. At
    /// A4 portrait, scale 2, that is 28 rows to a page rather than 14 — the same paper holding twice as
    /// much board, which is exactly what scaling down buys.
    ///
    /// THIS IS NO LONGER A CONSTANT, and that is the seam worth watching: it is a function of the page
    /// AND the scale, so a client that hardcodes it agrees with the export only for the paper it was
    /// written against.
    #[must_use]
    pub fn rows_per_page(&self) -> u32 {
        ((f64::from(self.layout_page_h_px()) + GRID_MARGIN_PX) / (GRID_ROW_H_PX + GRID_MARGIN_PX))
            .floor() as u32
    }
}

/// The A4-portrait printable width in CSS px. Shorthand for the default geometry.
#[must_use]
pub fn a4_content_w_px() -> u32 {
    PageGeometry::a4_portrait().content_w_px()
}

/// The A4-portrait printable height in CSS px. Shorthand for the default geometry.
#[must_use]
pub fn a4_content_h_px() -> u32 {
    PageGeometry::a4_portrait().content_h_px()
}

/// The A4-portrait paper column's pixel width. Shorthand for the default geometry.
#[must_use]
pub fn report_layout_w_px() -> u32 {
    PageGeometry::a4_portrait().layout_w_px()
}

/// One A4-portrait page's height in layout pixels. Shorthand for the default geometry.
#[must_use]
pub fn report_layout_page_h_px() -> u32 {
    PageGeometry::a4_portrait().layout_page_h_px()
}

/// How many grid rows fit on one A4-portrait page. Shorthand for the default geometry.
#[must_use]
pub fn a4_rows_per_page() -> u32 {
    PageGeometry::a4_portrait().rows_per_page()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The parameterisation must not have moved the default by so much as a rounding step: the A4
    /// consts, the value type and the free-function shorthands must all say the same thing.
    #[test]
    fn the_default_geometry_is_a4_portrait_to_the_millimetre() {
        let g = PageGeometry::default();
        assert_eq!(g, PageGeometry::a4_portrait());
        assert_eq!(g.content_w_mm(), A4_CONTENT_W_MM);
        assert_eq!(g.content_h_mm(), A4_CONTENT_H_MM);
        assert_eq!(g.content_w_px(), a4_content_w_px());
        assert_eq!(g.content_h_px(), a4_content_h_px());
        assert_eq!(g.layout_w_px(), report_layout_w_px());
        assert_eq!(g.layout_page_h_px(), report_layout_page_h_px());
        assert_eq!(g.rows_per_page(), a4_rows_per_page());
    }

    /// Landscape swaps the PAPER and nothing else. The header band still sits on top, so the margins
    /// stay where they were — a rotation that carried them round would move the running header onto
    /// the side of the sheet.
    #[test]
    fn landscape_swaps_the_paper_but_not_the_margins() {
        let p = PageGeometry::a4_portrait();
        let l = p.landscape();
        assert_eq!(l.w_mm, p.h_mm);
        assert_eq!(l.h_mm, p.w_mm);
        assert_eq!(l.margin_x_mm, p.margin_x_mm);
        assert_eq!(l.margin_top_mm, p.margin_top_mm);
        assert_eq!(l.margin_bottom_mm, p.margin_bottom_mm);
        // A wider, shorter content box — and therefore fewer rows to a page.
        assert!(l.content_w_mm() > p.content_w_mm());
        assert!(l.content_h_mm() < p.content_h_mm());
        assert!(l.rows_per_page() < p.rows_per_page());
        // …and it round-trips.
        assert_eq!(l.landscape(), p);
    }

    /// …and the reduction is UNIFORM: every rect is exactly `1/scale` of the true-size millimetres it
    /// would have had. Asserted for every geometry, not just A4 — the scale is a property of the value
    /// now, so the property has to hold across the value's range.
    #[test]
    fn the_whole_board_reduces_by_exactly_the_print_scale() {
        for g in [
            PageGeometry::a4_portrait(),
            PageGeometry::a4_portrait().landscape(),
            PageGeometry {
                scale: 1.0,
                ..PageGeometry::a4_portrait()
            },
        ] {
            let mm_per_layout_px = g.content_w_mm() / f64::from(g.layout_w_px());
            let mm_per_true_px = g.content_w_mm() / f64::from(g.content_w_px());
            assert!(
                (mm_per_true_px / mm_per_layout_px - g.scale).abs() < 1e-3,
                "the reduction must be exactly the scale, got {}",
                mm_per_true_px / mm_per_layout_px
            );
        }
    }
}
