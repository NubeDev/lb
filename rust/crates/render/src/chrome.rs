//! The branded **page chrome** — the cover page, the optional table-of-contents index, and the
//! running header/footer bands.
//!
//! Split out of `pdf.rs`, which had grown past the FILE-LAYOUT limit doing two jobs: assembling the
//! template and compiling it, *and* authoring every piece of decoration around the body. This is the
//! decoration. It emits Typst markup strings and touches nothing else — `pdf.rs` decides where each
//! piece goes.

use crate::convert::typst_string;
use crate::model::Assembled;

/// The cover page: a logo (if any), a generous top spacer, the oversized title,
/// an accent rule, and the brand header/footer text laid out as cover metadata.
/// The whole page suppresses the running header/footer via a page-local override,
/// then breaks so the body starts on a fresh page.
pub(crate) fn cover_page(
    a: &Assembled,
    logo_path: Option<&str>,
    primary: &str,
    accent: &str,
    muted: &str,
    rule: &str,
) -> String {
    let mut out = String::new();
    // The cover carries no running header/footer of its own.
    out.push_str("#page(header: none, footer: none)[\n");

    // Logo at the top of the cover.
    if let Some(path) = logo_path {
        out.push_str(&format!(
            "#v(0.6cm)\n#image({}, height: 1.7cm)\n",
            typst_string(path)
        ));
    }

    // A flexible spacer floats the title block down to roughly the upper third,
    // so the cover reads as composed rather than top-loaded. The 1.1fr / 1.9fr
    // split keeps it above centre.
    out.push_str("#v(1.1fr)\n");

    // A small accent eyebrow above the title gives the cover a designed feel even
    // when the brand sets no header text.
    let eyebrow = if a.brand.header_text.trim().is_empty() {
        "DOCUMENT".to_owned()
    } else {
        a.brand.header_text.trim().to_uppercase()
    };
    out.push_str(&format!(
        "#text(size: 9.5pt, weight: \"bold\", fill: {accent}, tracking: 0.18em)[#{}]\n#v(0.5cm)\n",
        typst_string(&eyebrow)
    ));

    // The title, oversized, with an accent rule beneath it. Justification and
    // hyphenation are forced off here so the display title never stretches words
    // or breaks mid-word the way justified body copy does.
    out.push_str(&format!(
        "#block(width: 85%)[#par(justify: false)[#text(size: 30pt, weight: \"bold\", fill: {primary}, hyphenate: false)[#{}]]]\n",
        typst_string(&a.title)
    ));
    out.push_str("#v(0.6cm)\n");
    out.push_str(&format!(
        "#box(width: 3.5cm, line(length: 100%, stroke: 3pt + {accent}))\n"
    ));

    out.push_str("#v(1.9fr)\n");

    // Cover metadata pinned near the bottom: a hairline and the brand footer text.
    out.push_str(&format!(
        "#line(length: 100%, stroke: 0.5pt + {rule})\n#v(0.3cm)\n"
    ));
    if !a.brand.footer_text.trim().is_empty() {
        out.push_str(&format!(
            "#text(size: 9pt, fill: {muted})[#{}]\n",
            typst_string(a.brand.footer_text.trim())
        ));
    }
    out.push_str("]\n#pagebreak(weak: true)\n\n");
    out
}
/// An index (table of contents) block on its own page listing each page's title in
/// render order, with a leader-dotted layout. Followed by a page break so the body
/// starts fresh. Every page the caller passes is a real page (a deliberately-blank
/// spacer included), so all are numbered.
pub(crate) fn index_block(a: &Assembled, primary: &str, accent: &str, rule: &str) -> String {
    let mut rows = String::new();
    for (i, _page) in a.pages.iter().enumerate() {
        // index · title · dotted leader filling the rest of the line.
        rows.push_str(&format!(
            "#grid(columns: (auto, auto, 1fr), gutter: 0.7em, align: (left, left, bottom), text(fill: {accent}, weight: \"bold\")[{}], text[#{}], box(width: 100%, inset: (bottom: 3pt))[#repeat(gap: 4pt)[#text(fill: {rule})[.]]])\n#v(0.4cm)\n",
            i + 1,
            typst_string(&a.page_label(i))
        ));
    }
    if rows.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str(&format!(
        "#text(size: 20pt, weight: \"bold\", fill: {primary})[Contents]\n#v(0.3cm)\n"
    ));
    out.push_str(&format!(
        "#line(length: 100%, stroke: 0.5pt + {rule})\n#v(0.7cm)\n"
    ));
    out.push_str(&rows);
    out.push_str("#pagebreak(weak: true)\n\n");
    out
}
/// The running header (body pages): the brand header text in small muted caps with
/// a hairline rule beneath, or `none` when the brand left the header blank.
pub(crate) fn running_header(text: &str, muted: &str, rule: &str) -> String {
    if text.trim().is_empty() {
        return "none".to_owned();
    }
    format!(
        "[#text(size: 8pt, fill: {muted}, tracking: 0.08em)[#{}] #v(-0.4em) #line(length: 100%, stroke: 0.5pt + {rule})]",
        typst_string(text.trim())
    )
}
/// The running footer (body pages): a hairline rule above the brand footer text on
/// the left and a live `page / total` counter on the right. Returns `none` when
/// there is neither footer text nor page numbering.
pub(crate) fn running_footer(
    brand_text: &str,
    muted: &str,
    rule: &str,
    page_numbers: bool,
) -> String {
    let has_text = !brand_text.trim().is_empty();
    if !has_text && !page_numbers {
        return "none".to_owned();
    }
    let left = if has_text {
        format!(
            "text(size: 8pt, fill: {muted})[#{}]",
            typst_string(brand_text.trim())
        )
    } else {
        "[]".to_owned()
    };
    // `context` lets the counter read the resolved page/total at layout time.
    let right = if page_numbers {
        format!(
            "context text(size: 8pt, fill: {muted})[#counter(page).display(\"1 / 1\", both: true)]"
        )
    } else {
        "[]".to_owned()
    };
    format!(
        "[#line(length: 100%, stroke: 0.5pt + {rule}) #v(-0.2em) #grid(columns: (1fr, auto), {left}, {right})]"
    )
}
