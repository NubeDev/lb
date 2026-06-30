//! Extract internal references from a markdown body — the document-store scope's "make links a
//! relation" move (move 3). A markdown save writes `doclink` (doc→doc) and `embed` (doc→asset)
//! edges for the references it carries, so backlinks, broken-link detection, and orphan-asset
//! GC are edge queries — and so a reader's access to a linked/embedded target is **re-gated at
//! read**, never widened by the parent doc's readability (the load-bearing deny test).
//!
//! Canonical internal-link grammar (document-store scope open question — locked here):
//!   - **doc link** — `lb-doc://{id}` (the wikilink/relative forms render as external);
//!   - **asset embed** — `lb-asset://{id}` (the example-flow form, `![alt](lb-asset://id)`).
//!
//! Extraction is a deliberately simple, dependency-free scan: the URIs are opaque schemes, so a
//! substring search for the prefix up to the next whitespace/`)`/`"` is unambiguous and avoids
//! pulling in a markdown parser (the store stays rendering-agnostic — scope non-goal). One form
//! the resolver recognizes; the rest are the consumer's problem.

/// Every doc id referenced by `body` via `lb-doc://{id}`. Deduplicated, order-unspecified.
pub(crate) fn doc_links(body: &str) -> Vec<String> {
    collect(body, "lb-doc://")
}

/// Every asset id embedded by `body` via `lb-asset://{id}`. Deduplicated, order-unspecified.
pub(crate) fn asset_embeds(body: &str) -> Vec<String> {
    collect(body, "lb-asset://")
}

/// Scan `body` for every `{prefix}{id}` occurrence, where `id` runs until the first character
/// that is not URL-safe (`A-Za-z0-9-_./:` — permissive for the workspace id grammar). Returns
/// the deduplicated set.
fn collect(body: &str, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(hit) = body[from..].find(prefix) {
        let start = from + hit + prefix.len();
        let rest = &body[start..];
        let end = rest
            .char_indices()
            .take_while(|(_, c)| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':'))
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let id = &rest[..end];
        if !id.is_empty() && !out.iter().any(|e: &String| e == id) {
            out.push(id.to_string());
        }
        from = start + end.max(1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_doc_and_asset_refs() {
        let body = "See [[doc:x]] rendered as lb-doc://alarm-matrix for detail.\n\
                    ![wiring](lb-asset://cooler-wiring) and lb-asset://cooler-wiring again.";
        assert_eq!(doc_links(body), vec!["alarm-matrix".to_string()]);
        assert_eq!(asset_embeds(body), vec!["cooler-wiring".to_string()]);
    }

    #[test]
    fn ignores_external_links() {
        let body = "https://example.com/x and mailto:a@b.co are external.";
        assert!(doc_links(body).is_empty());
        assert!(asset_embeds(body).is_empty());
    }
}
