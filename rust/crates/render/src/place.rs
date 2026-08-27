//! Emit one **placed** page — a page whose content is positioned rectangles rather than flowed
//! markdown.
//!
//! The markdown path ([`crate::convert`]) is a river: blocks flow down the page and Typst decides
//! where they land. A report page is a grid: the author put a panel at column 6, row 2, and it has to
//! be there. So a placed page is emitted as absolute `#place` boxes inside one full-size block,
//! bypassing the converter entirely. Both kinds of page coexist in one document — [`crate::pdf`]
//! picks per page — which is what lets a report keep the shipped cover, header/footer and index.

use crate::convert::typst_string;
use crate::geometry::PageGeometry;
use crate::model::Placement;

/// Render the placements of one page to Typst markup.
///
/// `resolve` maps a placement's `src` to the virtual image path registered for it, exactly as the
/// markdown converter's image resolver does. A placement whose `src` does not resolve renders an
/// **error tile** in situ — a bordered box carrying the panel's title and the reason — never an empty
/// hole and never a failed render. That is the whole point: a scheduled export whose browser could
/// not capture one panel still produces a PDF, and the gap is visible in it.
pub fn placed_page(
    placements: &[Placement],
    geo: &PageGeometry,
    resolve: impl Fn(&str) -> Option<String>,
) -> String {
    let mut out = String::new();
    // One full-content-box canvas; every child is positioned against its top-left corner.
    // The canvas is the CONTENT BOX of the page the placements were computed against — not a literal
    // A4 one. A mismatch here would put every rect in the right millimetre of the wrong box.
    out.push_str(&format!(
        "#block(width: {}mm, height: {}mm, breakable: false)[\n",
        geo.content_w_mm(),
        geo.content_h_mm()
    ));
    for p in placements {
        let inner = match resolve(&p.src) {
            Some(path) => format!(
                "#image({}, width: 100%, height: 100%, fit: \"contain\")",
                typst_string(&path)
            ),
            None => error_tile(&p.title, &p.note),
        };
        out.push_str(&format!(
            "  #place(top + left, dx: {:.3}mm, dy: {:.3}mm)[#box(width: {:.3}mm, height: {:.3}mm)[{inner}]]\n",
            p.rect.x, p.rect.y, p.rect.w, p.rect.h
        ));
    }
    out.push_str("]\n");
    out
}

/// The in-situ stand-in for a panel that could not be captured: a dashed, muted box naming the panel
/// and why it is empty. Deliberately loud enough to read as "this is missing", not as blank space.
fn error_tile(title: &str, note: &str) -> String {
    let title = typst_string(if title.is_empty() { "Panel" } else { title });
    let note = typst_string(if note.is_empty() {
        "not captured"
    } else {
        note
    });
    format!(
        "#box(width: 100%, height: 100%, stroke: (paint: luma(60%), dash: \"dashed\", thickness: 0.5pt), inset: 6pt, radius: 2pt)[\
#align(center + horizon)[#text(size: 9pt, fill: luma(45%))[*{title}*\\ {note}]]]"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell_rect::RectMm;

    fn placement(src: &str, title: &str) -> Placement {
        Placement {
            src: src.into(),
            title: title.into(),
            note: String::new(),
            rect: RectMm {
                x: 2.6,
                y: 2.6,
                w: 160.0,
                h: 60.0,
            },
        }
    }

    #[test]
    fn a_resolved_placement_emits_a_positioned_contained_image() {
        let out = placed_page(
            &[placement("snapshot:p1", "Energy")],
            &PageGeometry::a4_portrait(),
            |_| Some("img-0.png".into()),
        );
        assert!(out.contains("#place(top + left"), "positioned: {out}");
        assert!(out.contains("dx: 2.600mm"), "carries its x: {out}");
        assert!(out.contains("dy: 2.600mm"), "carries its y: {out}");
        assert!(
            out.contains("fit: \"contain\""),
            "never distorts the capture: {out}"
        );
        assert!(out.contains("img-0.png"), "resolved path: {out}");
    }

    #[test]
    fn an_unresolved_placement_becomes_a_titled_error_tile_not_a_hole() {
        let out = placed_page(
            &[placement("snapshot:missing", "Chiller load")],
            &PageGeometry::a4_portrait(),
            |_| None,
        );
        assert!(!out.contains("#image("), "no image is emitted: {out}");
        assert!(out.contains("Chiller load"), "names the panel: {out}");
        assert!(out.contains("not captured"), "states the reason: {out}");
        // The box is still placed, so the page keeps its shape and the gap is where the panel was.
        assert!(out.contains("#place(top + left"), "still positioned: {out}");
    }

    #[test]
    fn an_empty_page_still_emits_a_full_size_canvas() {
        let out = placed_page(&[], &PageGeometry::a4_portrait(), |_| None);
        assert!(out.contains("height: 251mm"), "one page tall: {out}");
    }
}
