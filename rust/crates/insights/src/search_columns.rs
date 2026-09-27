//! The roster columns the search box searches, and the indexes behind them
//! (`docs/scope/insights/insight-indexes-scope.md` §"Search by column").
//!
//! **The columns are the embedder's, never lb's.** Which columns a deployment's table shows is
//! product vocabulary, so lb names none: the node is handed a list at boot
//! (`BootConfig::insight_search_columns`), each column the text sources its cell falls back through
//! (`text_expr.rs`: `title` or `tag:<key>`), and the host copies it onto the filter
//! (`ListFilter::search_columns`, never read from the wire). An empty list searches the title alone.
//!
//! **What a search means:** the typed text appears ANYWHERE in a column's value as the cell shows it
//! (case ignored). Two parts do that (`filter_sql.rs`):
//!
//! 1. **Candidates from the indexes.** One full-text index per source field (the title, each tag key)
//!    with an `ngram(1,15)` analyzer, so any piece of a word is indexed: the middle of a word
//!    (`raka` in Pooraka), its end (`line` in Flatline), and a one-character word (`3` in "Lot 3").
//!    The earlier `edgengram(2,15)` indexed only word STARTS, and missed all three (measured on a
//!    live store). The indexes only narrow the rows; they can over-match (two pieces from different
//!    places) but never miss a row that contains the text.
//! 2. **The exact check on those candidates.** `string::contains` over each column's shown value
//!    (`text_expr::column_text`), so a row matches only if the text is really in a cell, and never
//!    on a source the cell does not show (a title hidden behind a `short_name`).
//!
//! **The CALLER builds the indexes, once** ([`ensure_search_indexes`]), before a search: `list` does
//! not. Building them inside every search made concurrent first searches conflict and fail (the
//! host's `insight/search_schema.rs` has the measurement).
//!
//! One responsibility: validate the configured columns and ensure their indexes.

use lb_store::{Store, StoreError};

use crate::error::InsightsError;
use crate::insight::OCC_TABLE;
use crate::schema::ANALYZER;
use crate::text_expr::{text_source, TextSource};

/// The most columns one node may search.
pub const MAX_SEARCH_COLUMNS: usize = 8;
/// The most sources one column may fall back through.
const MAX_SOURCES: usize = 4;
/// The most distinct tag keys across all columns. Each is a real full-text index (substring
/// indexing is several times the plain size) and one more `OR` branch on every search.
const MAX_TAG_KEYS: usize = 16;

/// Refuse a column list the search could not use safely. Called at boot, so a typo in the config
/// stops the node with a message rather than silently searching less than the operator configured.
pub fn validate_search_columns(columns: &[Vec<String>]) -> Result<(), InsightsError> {
    let bad = |m: String| {
        Err(InsightsError::BadInput(format!(
            "insight search columns: {m}"
        )))
    };
    if columns.len() > MAX_SEARCH_COLUMNS {
        return bad(format!(
            "at most {MAX_SEARCH_COLUMNS} columns, got {}",
            columns.len()
        ));
    }
    for column in columns {
        if column.is_empty() || column.len() > MAX_SOURCES {
            return bad(format!(
                "a column needs 1 to {MAX_SOURCES} sources, got {column:?}"
            ));
        }
        if let Some(src) = column.iter().find(|s| text_source(s).is_none()) {
            return bad(format!(
                "{src:?} is not `title` or `tag:<key>` with a lowercase identifier key"
            ));
        }
    }
    let keys = tag_keys(columns);
    if keys.len() > MAX_TAG_KEYS {
        return bad(format!(
            "at most {MAX_TAG_KEYS} distinct tag keys, got {}",
            keys.len()
        ));
    }
    Ok(())
}

/// The distinct tag keys the columns read, in first-seen order.
pub(crate) fn tag_keys(columns: &[Vec<String>]) -> Vec<&str> {
    let mut keys: Vec<&str> = Vec::new();
    for src in columns.iter().flatten() {
        if let Some(TextSource::Tag(k)) = text_source(src) {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    keys
}

/// Define the full-text index for each tag key the columns read, in `ws` (the title's index is
/// `schema.rs`'s). Idempotent (`IF NOT EXISTS`), so safe to retry; slow on the first run (it builds
/// over every stored row), so call it once per workspace, not per request.
///
/// The superseded prefix-only indexes are REMOVED first: with two full-text indexes on one field the
/// planner answers from the older one (`schema.rs` records how that was found).
pub async fn ensure_search_indexes(
    store: &Store,
    ws: &str,
    columns: &[Vec<String>],
) -> Result<(), StoreError> {
    // The analyzer and the title's index FIRST: the tag indexes name the analyzer, and on a store
    // upgraded from an older analyzer nothing else may have defined it yet (found live: the build
    // failed with "analyzer does not exist" until the next raise).
    crate::schema::ensure_insight_schema(store, ws).await?;
    let ddl: String = tag_keys(columns)
        .into_iter()
        .map(|k| {
            format!(
                "REMOVE INDEX IF EXISTS insight_tag_{k}_v2 ON TABLE {OCC_TABLE}; \
                 DEFINE INDEX IF NOT EXISTS {} ON TABLE {OCC_TABLE} \
                 FIELDS data.tags.{k} FULLTEXT ANALYZER {ANALYZER} BM25;",
                index_name(k)
            )
        })
        .collect();
    if ddl.is_empty() {
        return Ok(());
    }
    store.query_ws(ws, &ddl, vec![]).await?;
    Ok(())
}

/// The index for one key, versioned with the analyzer for the reason `schema.rs` gives: a changed
/// definition must be a NEW object, or `IF NOT EXISTS` keeps the old one for ever.
fn index_name(key: &str) -> String {
    format!("insight_tag_{key}_v3")
}

/// A plain lowercase identifier (a safe SurrealQL field name).
pub(crate) fn plain_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_lowercase())
        && chars.all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols(cs: &[&[&str]]) -> Vec<Vec<String>> {
        cs.iter()
            .map(|c| c.iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn columns_of_title_and_tags_pass() {
        let c = cols(&[
            &["tag:short_name", "tag:insight", "title"],
            &["tag:site", "tag:building"],
        ]);
        assert!(validate_search_columns(&c).is_ok());
        assert!(validate_search_columns(&[]).is_ok());
        assert_eq!(tag_keys(&c), ["short_name", "insight", "site", "building"]);
    }

    #[test]
    fn a_source_that_could_break_the_sql_is_refused() {
        for bad in [
            "tag:Site",
            "tag:site;DROP",
            "site",
            "data.title",
            "tag:",
            "severity",
        ] {
            assert!(
                validate_search_columns(&cols(&[&[bad]])).is_err(),
                "{bad:?} must be refused"
            );
        }
        assert!(
            validate_search_columns(&cols(&[&[]])).is_err(),
            "an empty column"
        );
    }

    #[test]
    fn oversized_lists_are_refused() {
        let many: Vec<Vec<String>> = (0..=MAX_SEARCH_COLUMNS)
            .map(|_| vec!["title".into()])
            .collect();
        assert!(validate_search_columns(&many).is_err());
        let keys: Vec<Vec<String>> = (0..=MAX_TAG_KEYS)
            .map(|i| vec![format!("tag:k{i}")])
            .collect();
        assert!(validate_search_columns(&keys[..MAX_SEARCH_COLUMNS]).is_ok());
        let wide: Vec<Vec<String>> = keys
            .chunks(MAX_SOURCES)
            .map(|c| c.iter().map(|v| v[0].clone()).collect())
            .collect();
        assert!(
            validate_search_columns(&wide).is_err(),
            "more than {MAX_TAG_KEYS} keys"
        );
    }
}
