//! Tests for [`super::compose`] — the cell grid → positioned pages composition.
//!
//! Split out of `compose.rs` to keep that file inside the FILE-LAYOUT ceiling; it is the same module
//! (`#[path]`-included), so it still reaches the crate-private vocabulary the composition uses.

use super::*;
use lb_render::geometry::PageGeometry;

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
    // is page 2. Expressed in terms of `PageGeometry::a4_portrait().rows_per_page()` rather than its current value, so the
    // print scale can move without rewriting the banding contract.
    let per = PageGeometry::a4_portrait().rows_per_page();
    let cells = vec![
        cell("a", 0, 0, 6, 5, "A"),
        cell("b", 6, 0, 6, 5, "B"),
        cell("c", 0, per, 12, 5, "C"),
        cell("d", 0, per * 2 + 2, 12, 5, "D"),
    ];
    let (pages, _) = compose_pages(&cells, &[], &PageGeometry::a4_portrait());
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
        cell(
            "b",
            0,
            PageGeometry::a4_portrait().rows_per_page() * 2,
            12,
            5,
            "B",
        ),
    ];
    let (pages, _) = compose_pages(&cells, &[], &PageGeometry::a4_portrait());
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
    let (pages, _) = compose_pages(&cells, &[], &PageGeometry::a4_portrait());
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
    let (pages, images) = compose_pages(&cells, &panels, &PageGeometry::a4_portrait());
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
    let (pages, _) = compose_pages(
        &cells,
        &[shot("shown", 0, 0, 6, 5, &[1u8])],
        &PageGeometry::a4_portrait(),
    );
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
        &PageGeometry::a4_portrait(),
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
    let (pages, images) = compose_pages(&cells, &panels, &PageGeometry::a4_portrait());
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
    let (record_driven, _) = compose_pages(
        &cells,
        &[legacy("meter", &[1u8])],
        &PageGeometry::a4_portrait(),
    );
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
    let (record_driven, _) =
        compose_pages(&cells, &[legacy("p", &[1u8])], &PageGeometry::a4_portrait());
    let (rendered_driven, _) = compose_pages(
        &cells,
        &[shot("p", 0, 0, 6, 5, &[1u8])],
        &PageGeometry::a4_portrait(),
    );
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
    let (pages, images) = compose_pages(
        &cells,
        &[legacy("a", &[1u8]), legacy("b", &[2u8])],
        &PageGeometry::a4_portrait(),
    );
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
    let (pages, _) = compose_pages(
        &cells,
        &[shot("a", 0, 0, 12, 5, &[1u8])],
        &PageGeometry::a4_portrait(),
    );
    assert_eq!(pages.len(), 1, "one rendered band ⇒ one page, not three");
}

#[test]
fn the_page_title_is_its_first_cell_and_an_untitled_board_leaves_it_empty() {
    let (pages, _) = compose_pages(
        &[cell("a", 0, 0, 12, 5, "Energy use")],
        &[],
        &PageGeometry::a4_portrait(),
    );
    assert_eq!(pages[0].title, "Energy use");
    let (pages, _) = compose_pages(
        &[cell("a", 0, 0, 12, 5, "")],
        &[],
        &PageGeometry::a4_portrait(),
    );
    assert_eq!(pages[0].title, "", "the renderer falls back to 'Page N'");
}

#[test]
fn a_report_with_no_cells_still_composes_one_page() {
    let (pages, images) = compose_pages(&[], &[], &PageGeometry::a4_portrait());
    assert_eq!(pages.len(), 1);
    assert!(pages[0].placements.is_empty());
    assert!(images.is_empty());
}

/// THE AUTHORED BREAK, at the compose level: the marker must not only move the page, it must move
/// the ORIGIN the rect is measured from. A page assignment that changed without the origin
/// following it is the regression that drew a flowed panel at the vertical offset it had on the
/// page it left.
#[test]
fn a_marked_cell_starts_its_own_page_and_is_measured_from_that_pages_top() {
    let png = [1_u8, 2, 3];
    let cells = vec![
        cell("cover", 0, 0, 12, 4, "Cover"),
        Cell {
            page_break_before: true,
            ..cell("trend", 0, 4, 12, 4, "Trend")
        },
    ];
    let panels = vec![
        shot("cover", 0, 0, 12, 4, &png),
        shot("trend", 0, 4, 12, 4, &png),
    ];

    let (pages, _) = compose_pages(&cells, &panels, &PageGeometry::a4_portrait());
    assert_eq!(
        pages.len(),
        2,
        "the marker adds a page — both bands fit one"
    );
    assert_eq!(pages[0].placements.len(), 1);
    assert_eq!(pages[1].placements.len(), 1);

    // The moved band is drawn from ITS page's top, so its rect is the one row 0 would have had.
    let top_of_page = pages[0].placements[0].rect.y;
    assert!(
        (pages[1].placements[0].rect.y - top_of_page).abs() < 1e-9,
        "a band that starts a page must be measured from that page's origin, got {} vs {}",
        pages[1].placements[0].rect.y,
        top_of_page
    );
}

/// Without the marker the same board is one page — so the test above is measuring the marker and
/// not some accident of the fixture.
#[test]
fn the_same_board_without_the_marker_is_a_single_page() {
    let png = [1_u8, 2, 3];
    let cells = vec![
        cell("cover", 0, 0, 12, 4, "Cover"),
        cell("trend", 0, 4, 12, 4, "Trend"),
    ];
    let panels = vec![
        shot("cover", 0, 0, 12, 4, &png),
        shot("trend", 0, 4, 12, 4, &png),
    ];
    let (pages, _) = compose_pages(&cells, &panels, &PageGeometry::a4_portrait());
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].placements.len(), 2);
}

/// A shorter page composes the same board into more pages — the geometry parameter is threaded
/// all the way through placement, not just through the page setup.
#[test]
fn a_shorter_page_composes_the_same_board_into_more_pages() {
    let png = [1_u8, 2, 3];
    let cells = vec![cell("a", 0, 0, 12, 10, "A"), cell("b", 0, 10, 12, 10, "B")];
    let panels = vec![
        shot("a", 0, 0, 12, 10, &png),
        shot("b", 0, 10, 12, 10, &png),
    ];

    let portrait = compose_pages(&cells, &panels, &PageGeometry::a4_portrait()).0;
    let landscape = compose_pages(&cells, &panels, &PageGeometry::a4_portrait().landscape()).0;
    assert_eq!(portrait.len(), 1);
    assert!(
        landscape.len() > portrait.len(),
        "fewer rows to a landscape page ⇒ more pages"
    );
}
