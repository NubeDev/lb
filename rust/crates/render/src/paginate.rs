//! Which A4 PAGE a board row lands on — the report's pagination rule.
//!
//! Split from [`super::geometry`], which answers the other half of the question: given a page, WHERE on
//! it does a cell sit. Paging is about bands and fit; placement is about millimetres. They share only
//! the page box, so they read better apart than as one 470-line file.
//!
//! The rule in one line: a row band lands on the page the author put it on, unless it would not FIT
//! there, in which case it flows whole onto the next one. Never squashed, never compacted.

use super::geometry::a4_rows_per_page;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_breaks_fall_every_whole_page_of_rows() {
        let per = a4_rows_per_page();
        assert_eq!(page_of_row(0), 0);
        assert_eq!(page_of_row(per - 1), 0);
        assert_eq!(page_of_row(per), 1);
        assert_eq!(page_of_row(per * 2 - 1), 1);
        assert_eq!(page_of_row(per * 2), 2);
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
    fn paginate_is_empty_for_no_rows() {
        assert!(paginate(&[]).is_empty());
    }

    #[test]
    fn deliberate_blank_pages_are_preserved_not_compacted() {
        // Nothing occupies page 1; a band two whole pages down belongs on page 2 and must stay there.
        let late_y = a4_rows_per_page() * 2;
        let paged = paginate(&[(0, 5), (late_y, 5)]);
        assert_eq!(paged.iter().find(|r| r.y == 0).unwrap().page, 0);
        let late = paged.iter().find(|r| r.y == late_y).unwrap();
        assert_eq!(late.page, 2, "the author's blank page survives");
        assert_eq!(late.page_start_y, late_y);
    }
}
