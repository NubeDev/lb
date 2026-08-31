//! Which A4 PAGE a board row lands on — the report's pagination rule.
//!
//! Split from [`super::geometry`], which answers the other half of the question: given a page, WHERE on
//! it does a cell sit. Paging is about bands and fit; placement is about millimetres. They share only
//! the page box, so they read better apart than as one 470-line file.
//!
//! The rule in one line: a row band lands on the page the author put it on, unless it would not FIT
//! there, in which case it flows whole onto the next one — **and a band the author MARKED always
//! starts a new page**. Never squashed, never compacted.
//!
//! The marker is checked ahead of the fit test and moves a band FORWARD only, which is what keeps the
//! "deliberate blank pages are preserved" property true by construction.

use super::geometry::{PageGeometry, a4_rows_per_page};

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

/// One row band to paginate: where it starts, how tall it is, and whether the author marked it as
/// starting a new page.
///
/// A tuple would do for the first two, and did until the marker existed; naming the third is what stops
/// a caller passing `true` where a height belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Band {
    /// The board row the band starts at.
    pub y: u32,
    /// The band's height in grid rows.
    pub h: u32,
    /// The author's explicit "start a new page here" (`Cell::page_break_before`).
    pub break_before: bool,
}

impl Band {
    /// A band with no marker — the shape every caller used before page breaks were authorable.
    #[must_use]
    pub const fn new(y: u32, h: u32) -> Self {
        Self {
            y,
            h,
            break_before: false,
        }
    }
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
/// would overflow onto the next page instead of squashing it — and starting a new page wherever the
/// author marked one.
///
/// `bands` is one entry per cell; entries sharing a `y` are one band and are collapsed to the tallest,
/// so cells laid side by side (three KPI tiles on one row) always page together and never split
/// mid-band. **The marker is collapsed with OR**: a row breaks before it if ANY cell on that row
/// carries the flag, which is what makes "mark this KPI row" mean the row rather than one tile.
///
/// Why the fit rule exists: the fixed-band rule pages on `y / rows_per_page` alone. A 7-row panel
/// starting one row from a page's end is "on that page" because its top is — so the clamp turned it
/// into a sliver with an empty page underneath. Fitting the band moves it whole to the next page,
/// where it renders at its authored size. A band taller than a whole page still cannot fit anywhere;
/// it starts its own page and is clamped there, which is the honest "this panel is too tall" signal.
///
/// Why the marker sits AHEAD of the fit rule: an author who says "start a page here" is stating an
/// intent, and a fit test can only ever agree with it or be overruled by it. Checking fit first would
/// let a band that happens to fit stay put and silently drop the instruction.
///
/// Deliberate blank space is PRESERVED. The fixed band a row falls in is treated as a floor, so an
/// author who leaves a page's worth of empty rows to force a page break still gets that break; this
/// only ever moves a band *later* than the fixed rule would, never earlier. The marker obeys the same
/// direction — it can add a page, never remove one.
///
/// The returned entries are sorted by `y`, one per distinct row.
#[must_use]
pub fn paginate_with(geo: &PageGeometry, bands: &[Band]) -> Vec<PagedRow> {
    let per_page = geo.rows_per_page();
    if per_page == 0 || bands.is_empty() {
        return Vec::new();
    }

    // Collapse to one entry per row band: the tallest cell on that row, and the OR of its markers.
    let mut rows: Vec<Band> = Vec::new();
    for b in bands {
        let h = b.h.max(1);
        match rows.iter_mut().find(|r| r.y == b.y) {
            Some(r) => {
                r.h = r.h.max(h);
                r.break_before |= b.break_before;
            }
            None => rows.push(Band {
                y: b.y,
                h,
                break_before: b.break_before,
            }),
        }
    }
    rows.sort_unstable_by_key(|r| r.y);

    let mut out: Vec<PagedRow> = Vec::with_capacity(rows.len());
    let mut page = 0;
    let mut page_start_y = 0;

    for band in rows {
        // Start from where the AUTHOR put this band. Deliberate empty space is meaningful — a board
        // whose next panel sits a page down wants a blank page between, and compacting that away
        // would silently rewrite the author's layout. So the fixed band is a FLOOR, never a ceiling.
        let banded = band.y / per_page;
        if banded > page {
            page = banded;
            page_start_y = banded * per_page;
        }

        // Rows the band occupies on the current page, measured from that page's first row. `y` is
        // never below `page_start_y` because the bands are sorted and the floor only moves forward.
        let row_on_page = band.y - page_start_y;
        if band.break_before && row_on_page > 0 {
            // The author's instruction, ahead of the fit test. `row_on_page > 0` is what stops a
            // marked band that ALREADY starts a page from emitting a gratuitous blank one before it.
            page += 1;
            page_start_y = band.y;
        } else if row_on_page + band.h > per_page && row_on_page > 0 {
            // The fit rule: push to the next page when the band would run past the page's last row —
            // unless it already starts at the top, where no move can help and the clamp is honest.
            page += 1;
            page_start_y = band.y;
        }
        out.push(PagedRow {
            y: band.y,
            page,
            page_start_y,
        });
    }
    out
}

/// [`paginate_with`] on the A4-portrait default with no page-break markers — `(y, height)` pairs, the
/// shape every caller used before either was a parameter.
#[must_use]
pub fn paginate(rows: &[(u32, u32)]) -> Vec<PagedRow> {
    let bands: Vec<Band> = rows.iter().map(|&(y, h)| Band::new(y, h)).collect();
    paginate_with(&PageGeometry::a4_portrait(), &bands)
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

    /// THE FEATURE. A marked band starts a new page even though it would have fitted where it was.
    #[test]
    fn a_marked_band_starts_a_new_page_even_when_it_would_have_fitted() {
        let geo = PageGeometry::a4_portrait();
        let unmarked = paginate_with(&geo, &[Band::new(0, 4), Band::new(4, 4)]);
        assert_eq!(unmarked[1].page, 0, "it fits, so without a marker it stays");

        let marked = paginate_with(
            &geo,
            &[
                Band::new(0, 4),
                Band {
                    y: 4,
                    h: 4,
                    break_before: true,
                },
            ],
        );
        assert_eq!(marked[0].page, 0);
        assert_eq!(marked[1].page, 1, "the author said start a page");
        assert_eq!(
            marked[1].page_start_y, 4,
            "and the new page begins at ITS row, so it draws from that origin"
        );
    }

    /// A marked band that is ALREADY at the top of its page must not push itself onto the next one —
    /// that would print a blank sheet for an instruction the layout already satisfies.
    #[test]
    fn a_marked_band_already_at_a_page_top_adds_no_blank_page() {
        let geo = PageGeometry::a4_portrait();
        let per = geo.rows_per_page();

        // Row 0 is the top of page 0.
        let first = paginate_with(
            &geo,
            &[Band {
                y: 0,
                h: 4,
                break_before: true,
            }],
        );
        assert_eq!(first[0].page, 0);
        assert_eq!(first[0].page_start_y, 0);

        // …and a band that the FLOOR already moved to a fresh page is at a top too.
        let floored = paginate_with(
            &geo,
            &[
                Band::new(0, 4),
                Band {
                    y: per,
                    h: 4,
                    break_before: true,
                },
            ],
        );
        assert_eq!(floored[1].page, 1, "page 1, not page 2");
        assert_eq!(floored[1].page_start_y, per);
    }

    /// The marker moves a band FORWARD only. An author who left a whole blank page still has it — the
    /// marker can add a page, never remove one.
    #[test]
    fn a_marker_never_compacts_a_deliberate_blank_page() {
        let geo = PageGeometry::a4_portrait();
        let late = geo.rows_per_page() * 2;
        let paged = paginate_with(
            &geo,
            &[
                Band::new(0, 5),
                Band {
                    y: late,
                    h: 5,
                    break_before: true,
                },
            ],
        );
        // Page 2 by the floor, and the marker finds it already at a page top, so it stays page 2.
        assert_eq!(
            paged[1].page, 2,
            "the author's blank page survives the marker"
        );
        assert_eq!(paged[1].page_start_y, late);
    }

    /// A marked band too tall for ANY page still starts its own page and is left to the clamp — the
    /// marker must not send it hunting for room that does not exist.
    #[test]
    fn a_marked_band_taller_than_a_page_starts_its_page_and_stops_there() {
        let geo = PageGeometry::a4_portrait();
        let too_tall = geo.rows_per_page() + 6;
        let paged = paginate_with(
            &geo,
            &[
                Band::new(0, 4),
                Band {
                    y: 4,
                    h: too_tall,
                    break_before: true,
                },
            ],
        );
        assert_eq!(paged[1].page, 1);
        assert_eq!(
            paged[1].page_start_y, 4,
            "exactly one page forward, not two"
        );
    }

    /// The marker is a property of the ROW, not of one tile on it. Marking any cell of a KPI row
    /// breaks before the whole row — anything else would tear a band in half, which the collapse
    /// exists to prevent.
    #[test]
    fn a_marker_on_any_cell_of_a_row_breaks_before_the_whole_row() {
        let geo = PageGeometry::a4_portrait();
        let paged = paginate_with(
            &geo,
            &[
                Band::new(0, 4),
                Band::new(4, 2),
                Band {
                    y: 4,
                    h: 2,
                    break_before: true,
                },
                Band::new(4, 2),
            ],
        );
        assert_eq!(paged.len(), 2, "still one entry per row band");
        assert_eq!(paged[1].page, 1);
    }

    /// Pagination follows the PAGE it is given. Fewer rows to a landscape page ⇒ the same board needs
    /// more of them; the rule itself is unchanged.
    #[test]
    fn a_shorter_page_paginates_the_same_board_into_more_pages() {
        let portrait = PageGeometry::a4_portrait();
        let landscape = portrait.landscape();
        assert!(landscape.rows_per_page() < portrait.rows_per_page());

        let bands = [Band::new(0, 10), Band::new(10, 10)];
        let p = paginate_with(&portrait, &bands);
        let l = paginate_with(&landscape, &bands);
        assert_eq!(
            p.iter().map(|r| r.page).max(),
            Some(0),
            "both fit on A4 portrait"
        );
        assert!(
            l.iter().map(|r| r.page).max() > Some(0),
            "and they do not on the shorter page"
        );
    }
}
