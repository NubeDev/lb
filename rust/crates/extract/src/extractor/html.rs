//! The HTML→markdown extractor (doc-extraction scope) — a dependency-light, best-effort tag scan.
//! It deliberately pulls in NO markdown/HTML engine: HTML fidelity is explicitly best-effort in
//! v1 (the source media is the fidelity escape hatch), so a focused scanner over the common
//! structural tags (headings, paragraphs, lists, links, emphasis) is the right amount of machinery.
//! Determinism: the same bytes always produce the same markdown.
//!
//! The scanner is tolerant of HTML soup (unclosed tags, attributes, entities) because it never
//! builds a DOM — it walks tags and text, tracking only enough state (list depth, the current
//! link href) to place markdown markers. Anything it doesn't recognize becomes plain text.

use crate::error::ExtractError;
use crate::model::{ExtractOpts, ExtractedDoc};
use crate::trait_def::Extractor;

/// HTML → a single best-effort markdown doc.
pub struct HtmlExtractor;

impl Extractor for HtmlExtractor {
    fn id(&self) -> &'static str {
        "html"
    }

    fn version(&self) -> u32 {
        1
    }

    fn extract(
        &self,
        bytes: &[u8],
        _mime: &str,
        _opts: &ExtractOpts,
    ) -> Result<Vec<ExtractedDoc>, ExtractError> {
        let html = String::from_utf8_lossy(bytes);
        let title = extract_title(&html);
        // Convert only the <body> when present (so <head>'s title/meta text doesn't leak into the
        // body); fall back to the whole document for fragments with no <body>.
        let body = body_slice(&html);
        let markdown = to_markdown(body);
        Ok(vec![ExtractedDoc::whole(title, markdown)])
    }
}

/// The inside of `<body>…</body>` if present, else the whole document (an HTML fragment). Keeps
/// `<head>` metadata text out of the derived body.
fn body_slice(html: &str) -> &str {
    let lower = html.to_ascii_lowercase();
    let start = match lower.find("<body") {
        Some(b) => match lower[b..].find('>') {
            Some(gt) => b + gt + 1,
            None => return html,
        },
        None => return html,
    };
    let end = lower[start..]
        .find("</body")
        .map(|e| start + e)
        .unwrap_or(html.len());
    &html[start..end]
}

/// The `<title>` text if present, else the first `<h1>`, else empty — the doc's title hint.
fn extract_title(html: &str) -> String {
    tag_text(html, "title")
        .or_else(|| tag_text(html, "h1"))
        .map(|t| decode_entities(&collapse_ws(&t)))
        .unwrap_or_default()
}

/// The text between the first `<tag ...>` and `</tag>` (case-insensitive), raw (un-decoded).
fn tag_text(html: &str, tag: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag}");
    let start_tag = lower.find(&open)?;
    let gt = lower[start_tag..].find('>')? + start_tag + 1;
    let close = format!("</{tag}");
    let end = lower[gt..].find(&close).map(|i| i + gt)?;
    Some(html[gt..end].to_string())
}

/// Convert an HTML body to best-effort markdown. A single forward scan: emit markdown markers at
/// block/inline tag boundaries, pass text through (entity-decoded, whitespace-collapsed), and drop
/// the contents of `<script>`/`<style>` wholesale.
fn to_markdown(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    let mut list_depth: usize = 0;
    let mut link_href: Option<String> = None;
    let mut skip_until: Option<&'static str> = None;

    while i < bytes.len() {
        if bytes[i] == b'<' {
            let Some(close_rel) = html[i..].find('>') else {
                break;
            };
            let raw = &html[i + 1..i + close_rel];
            i += close_rel + 1;
            let name = tag_name(raw);

            // Inside <script>/<style>: swallow everything until the matching close tag. The name
            // of a close tag is `/script` (the leading slash is part of the tag name), so match on
            // the raw slice starting with `/` + the element.
            if let Some(end) = skip_until {
                if raw.starts_with('/') && name[1..].eq_ignore_ascii_case(end) {
                    skip_until = None;
                }
                continue;
            }

            match name.to_ascii_lowercase().as_str() {
                "script" => skip_until = Some("script"),
                "style" => skip_until = Some("style"),
                "h1" => out.push_str("\n\n# "),
                "h2" => out.push_str("\n\n## "),
                "h3" => out.push_str("\n\n### "),
                "h4" => out.push_str("\n\n#### "),
                "h5" | "h6" => out.push_str("\n\n##### "),
                "/h1" | "/h2" | "/h3" | "/h4" | "/h5" | "/h6" => out.push_str("\n\n"),
                "p" | "/p" | "br" | "br/" | "div" | "/div" => push_break(&mut out),
                "ul" | "ol" => list_depth += 1,
                "/ul" | "/ol" => {
                    list_depth = list_depth.saturating_sub(1);
                    push_break(&mut out);
                }
                "li" => {
                    push_break(&mut out);
                    for _ in 1..list_depth.max(1) {
                        out.push_str("  ");
                    }
                    out.push_str("- ");
                }
                "strong" | "b" => out.push_str("**"),
                "/strong" | "/b" => out.push_str("**"),
                "em" | "i" => out.push('*'),
                "/em" | "/i" => out.push('*'),
                "a" => link_href = href_of(raw),
                "/a" => {
                    if let Some(href) = link_href.take() {
                        out.push_str(&format!("]({href})"));
                    }
                }
                _ => {}
            }
            if name.eq_ignore_ascii_case("a") && link_href.is_some() {
                out.push('[');
            }
        } else {
            // Text run up to the next tag.
            let next = html[i..].find('<').map(|r| i + r).unwrap_or(bytes.len());
            if skip_until.is_none() {
                let text = decode_entities(&collapse_ws(&html[i..next]));
                out.push_str(&text);
            }
            i = next;
        }
    }
    tidy(&out)
}

/// The tag name (letters/`/`) from the raw inner-`<...>` slice, up to the first space/attr.
fn tag_name(raw: &str) -> &str {
    let end = raw.find(|c: char| c.is_whitespace()).unwrap_or(raw.len());
    raw[..end].trim_end_matches('/')
}

/// The `href="..."` (or `href='...'`) value of an `<a>` tag's raw inner slice.
fn href_of(raw: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let at = lower.find("href")?;
    let rest = &raw[at + 4..];
    let eq = rest.find('=')?;
    let after = rest[eq + 1..].trim_start();
    let (quote, body) = match after.chars().next()? {
        q @ ('"' | '\'') => (Some(q), &after[1..]),
        _ => (None, after),
    };
    let end = match quote {
        Some(q) => body.find(q)?,
        None => body.find(|c: char| c.is_whitespace()).unwrap_or(body.len()),
    };
    Some(body[..end].to_string())
}

/// Collapse runs of ASCII whitespace to single spaces (HTML is whitespace-insensitive in text).
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

/// Decode the handful of named + numeric entities that matter for readable text.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        if let Some(semi) = tail.find(';').filter(|&p| p <= 8) {
            let ent = &tail[1..semi];
            let decoded = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" | "#39" => Some('\''),
                "nbsp" => Some(' '),
                _ => ent
                    .strip_prefix('#')
                    .and_then(|n| n.parse::<u32>().ok())
                    .and_then(char::from_u32),
            };
            match decoded {
                Some(c) => {
                    out.push(c);
                    rest = &tail[semi + 1..];
                    continue;
                }
                None => {
                    out.push('&');
                    rest = &tail[1..];
                    continue;
                }
            }
        }
        out.push('&');
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

/// Append a soft paragraph break without piling up blank lines.
fn push_break(out: &mut String) {
    if !out.ends_with("\n\n") && !out.is_empty() {
        out.push_str("\n\n");
    }
}

/// Collapse 3+ newlines to a paragraph break, strip trailing spaces per line, and trim the ends —
/// a clean markdown body.
fn tidy(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut newlines = 0;
    let mut pending_spaces = 0usize;
    for c in s.chars() {
        if c == '\n' {
            pending_spaces = 0; // drop trailing spaces before a newline
            newlines += 1;
            if newlines <= 2 {
                out.push('\n');
            }
        } else if c == ' ' {
            pending_spaces += 1;
        } else {
            for _ in 0..pending_spaces {
                out.push(' ');
            }
            pending_spaces = 0;
            newlines = 0;
            out.push(c);
        }
    }
    out.trim().to_string()
}
