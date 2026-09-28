//! A roster column's TEXT as SQL — the first non-empty of its sources, lowercased — shared by the
//! column sort (`sort_sql.rs`) and the column search (`filter_sql.rs`), so a column sorts and is
//! searched by exactly the value its cell shows.
//!
//! A text source is `title` or `tag:<key>`. lb names no column: the embedder lists the sources, in
//! the order its cell falls back through them (a Name cell showing `short_name`, else the `insight`
//! tag, else the title is `["tag:short_name", "tag:insight", "title"]`).
//!
//! One responsibility: parse a text source and render a column's value.

use crate::search_columns::plain_ident;

/// One text source of a column.
pub(crate) enum TextSource<'a> {
    Title,
    Tag(&'a str),
}

/// Parse a text source; `None` for anything else (a numeric source, a bad key, a typo).
pub(crate) fn text_source(s: &str) -> Option<TextSource<'_>> {
    if s == "title" {
        return Some(TextSource::Title);
    }
    s.strip_prefix("tag:")
        .filter(|k| plain_ident(k))
        .map(TextSource::Tag)
}

/// The first non-empty source, lowercased; `''` when every source is empty. Sources that do not
/// parse are skipped, so nothing unvalidated is spliced into SQL.
pub(crate) fn column_text(sources: &[String]) -> String {
    // Built right to left: IF a is non-empty THEN a ELSE (the rest).
    let mut expr = "''".to_string();
    for src in sources.iter().rev() {
        let field = match text_source(src) {
            Some(TextSource::Tag(k)) => format!("data.tags.{k}"),
            Some(TextSource::Title) => "data.title".to_string(),
            None => continue,
        };
        expr = format!("(IF {field} != NONE AND {field} != '' THEN {field} ELSE {expr} END)");
    }
    format!("string::lowercase({expr})")
}
