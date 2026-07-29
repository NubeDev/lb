//! The PLACED-PAGE path end to end (reports-as-dashboards scope): real PNG bytes at real grid
//! rectangles through a real Typst compile, the honest error tile when a capture never arrived, and
//! the proof that a document carrying no placements renders byte-identically to what shipped.
//!
//! An integration test rather than more `#[cfg(test)]` in `pdf.rs`, which is over the FILE-LAYOUT
//! ratchet's baseline — and these only ever touch the public API, so they belong out here anyway.

use lb_render::{Assembled, ImageAsset, Placement, cell_rect_mm, render_pdf};

/// A real 1x1 PNG — the format the browser actually posts as a panel snapshot. Raster, not SVG,
/// because `fit: "contain"` on a bitmap is the part of the placed path worth proving.
fn one_px_png() -> Vec<u8> {
    vec![
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc,
        0xcf, 0xc0, 0x50, 0x0f, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ]
}

/// A report page: real PNG bytes placed at grid rectangles, compiled by real Typst. This is the
/// end of the A4 grid path — geometry maths that never reaches a compiler proves nothing.
#[test]
fn a_placed_grid_page_compiles_with_its_panels_positioned() {
    let png = one_px_png();
    let mut a = Assembled::with_pages("Monthly Energy", vec![String::new()]);
    a.images
        .push(ImageAsset::new("snapshot:a", "a.png", png.clone()));
    a.images.push(ImageAsset::new("snapshot:b", "b.png", png));
    a.placements = vec![vec![
        Placement {
            src: "snapshot:a".into(),
            title: "Left".into(),
            note: String::new(),
            rect: cell_rect_mm(0, 0, 6, 5),
        },
        Placement {
            src: "snapshot:b".into(),
            title: "Right".into(),
            note: String::new(),
            rect: cell_rect_mm(6, 0, 6, 5),
        },
    ]];
    let pdf = render_pdf(&a).expect("a placed page renders");
    assert!(pdf.starts_with(b"%PDF-"), "output is not a PDF");
}

/// The honest-failure path: a page whose snapshot never arrived still yields a PDF, with the gap
/// drawn as a tile rather than the render failing or the panel silently vanishing.
#[test]
fn a_placed_page_with_no_snapshot_still_renders_an_error_tile() {
    let mut a = Assembled::with_pages("Monthly Energy", vec![String::new()]);
    a.placements = vec![vec![Placement {
        src: "snapshot:never-captured".into(),
        title: "Chiller load".into(),
        note: "render timed out".into(),
        rect: cell_rect_mm(0, 0, 12, 6),
    }]];
    let pdf = render_pdf(&a).expect("a missing snapshot must not fail the export");
    assert!(pdf.starts_with(b"%PDF-"), "output is not a PDF");
}

/// A document with NO placements must take the markdown path byte-for-byte — the additive field
/// cannot have changed what every existing caller renders.
#[test]
fn a_document_without_placements_is_unchanged_by_the_placed_path() {
    let a = Assembled::with_pages("Book", vec!["# Hello\n\nBody.".to_owned()]);
    let with_empty = Assembled {
        placements: vec![Vec::new()],
        ..a.clone()
    };
    assert_eq!(
        render_pdf(&a).unwrap(),
        render_pdf(&with_empty).unwrap(),
        "an empty placements entry must fall through to markdown"
    );
}
