//! The **letterhead** — four caller-authored corners printed in the running header and footer of
//! every body page (report-letterhead).
//!
//! The caller resolves its own variables before it gets here; this file knows only the three tokens
//! that cannot be resolved anywhere else, because they are facts about the laid-out PDF: the logo
//! (bytes the renderer holds), the page number and the page total (known at layout time). Everything
//! else in a corner is TEXT and is emitted as a Typst string literal, never as markup, so a corner
//! reading `#line` prints `#line`.
//!
//! Emits Typst strings and touches nothing else, like `chrome.rs` beside it.

use crate::convert::typst_string;

/// The three tokens only the renderer can resolve. Spelled in the caller's own variable grammar
/// (`${name}`) so an author sees one syntax, and `__`-prefixed so they cannot collide with a
/// variable the caller owns.
const TOKENS: [&str; 3] = ["${__logo}", "${__report.page}", "${__report.pages}"];

/// One corner as a Typst CODE block: each piece is a statement and the block joins them into
/// content. Code mode rather than markup because a string literal is the only escaping a corner
/// then needs — there is no markup for a caller's text to be mistaken for.
pub(crate) fn corner(text: &str, logo_path: Option<&str>) -> String {
    let mut out = String::from("{ ");
    for (i, line) in text.trim().lines().enumerate() {
        if i > 0 {
            out.push_str("linebreak(); ");
        }
        let mut rest = line;
        while !rest.is_empty() {
            // The earliest token in what is left; ties cannot happen (no token prefixes another at
            // the same offset except page/pages, and `find` on the longer one only matches itself).
            let next = TOKENS
                .iter()
                .filter_map(|t| rest.find(t).map(|at| (at, *t)))
                .min_by_key(|(at, t)| (*at, usize::MAX - t.len()));
            let Some((at, token)) = next else {
                out.push_str(&format!("{}; ", typst_string(rest)));
                break;
            };
            if at > 0 {
                out.push_str(&format!("{}; ", typst_string(&rest[..at])));
            }
            match token {
                "${__logo}" => {
                    // No logo on the brand ⇒ the token prints nothing, like an empty variable.
                    if let Some(path) = logo_path {
                        out.push_str(&format!(
                            "box(image({}, height: 0.9cm)); ",
                            typst_string(path)
                        ));
                    }
                }
                "${__report.page}" => out.push_str("(context counter(page).display()); "),
                _ => out.push_str("(context str(counter(page).final().first())); "),
            }
            rest = &rest[at + token.len()..];
        }
    }
    out.push('}');
    out
}

/// How a band's text is set: the brand's body font and a muted tint of its text colour.
///
/// The font is passed EXPLICITLY rather than inherited. A page band takes its text style from the
/// page it decorates, and a report page is placed images with no flowed text at all — on such a page
/// the document's `set text(font: ..)` never reaches the band and it falls back to the renderer's
/// default face, whatever the brand says.
pub(crate) struct BandStyle<'a> {
    /// A Typst font-list expression, e.g. `("Inter", "Libertinus Serif")`.
    pub font: &'a str,
    /// A Typst colour expression for the text.
    pub muted: &'a str,
    /// A Typst colour expression for the hairline.
    pub rule: &'a str,
}

/// The two corners of one band side by side: left-aligned and right-aligned halves.
fn pair(left: &str, right: &str, logo_path: Option<&str>, style: &BandStyle) -> String {
    let BandStyle { font, muted, .. } = style;
    format!(
        "#grid(columns: (1fr, 1fr), align: (left + horizon, right + horizon), text(font: {font}, size: 8pt, fill: {muted}, {}), text(font: {font}, size: 8pt, fill: {muted}, {}))",
        corner(left, logo_path),
        corner(right, logo_path)
    )
}

/// The letterhead's top band, with a hairline beneath it — or `none` when both corners are empty.
pub(crate) fn header_band(
    left: &str,
    right: &str,
    logo_path: Option<&str>,
    style: &BandStyle,
) -> String {
    if left.trim().is_empty() && right.trim().is_empty() {
        return "none".to_owned();
    }
    format!(
        "[{} #v(-0.4em) #line(length: 100%, stroke: 0.5pt + {})]",
        pair(left, right, logo_path, style),
        style.rule
    )
}

/// The letterhead's bottom band, under a hairline — or `none` when both corners are empty.
pub(crate) fn footer_band(
    left: &str,
    right: &str,
    logo_path: Option<&str>,
    style: &BandStyle,
) -> String {
    if left.trim().is_empty() && right.trim().is_empty() {
        return "none".to_owned();
    }
    format!(
        "[#line(length: 100%, stroke: 0.5pt + {}) #v(-0.2em) {}]",
        style.rule,
        pair(left, right, logo_path, style)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_a_string_literal_and_lines_break() {
        let c = corner("Acme \"Pty\"\nLevel 4", None);
        assert_eq!(c, r#"{ "Acme \"Pty\""; linebreak(); "Level 4"; }"#);
    }

    #[test]
    fn the_page_tokens_become_layout_time_counters() {
        let c = corner("Page ${__report.page} of ${__report.pages}", None);
        assert!(
            c.contains(r#""Page "; (context counter(page).display());"#),
            "{c}"
        );
        assert!(
            c.contains(r#"" of "; (context str(counter(page).final().first()));"#),
            "{c}"
        );
    }

    #[test]
    fn the_logo_token_is_the_image_when_there_is_one_and_nothing_when_not() {
        assert!(corner("${__logo}", Some("logo.svg")).contains(r#"image("logo.svg""#));
        assert_eq!(corner("${__logo}", None), "{ }");
    }

    #[test]
    fn an_unknown_reference_prints_as_typed() {
        assert_eq!(corner("${site}", None), r#"{ "${site}"; }"#);
    }

    #[test]
    fn a_band_with_two_empty_corners_is_no_band() {
        let style = BandStyle {
            font: "(\"F\",)",
            muted: "m",
            rule: "r",
        };
        assert_eq!(header_band("", "  ", None, &style), "none");
        assert_eq!(footer_band("", "", None, &style), "none");
        assert_ne!(footer_band("x", "", None, &style), "none");
    }
}
