//! The tag keys the roster's search box matches BESIDE the title, and the indexes behind them
//! (`docs/scope/insights/insight-indexes-scope.md` §"Search past the name").
//!
//! **The keys are the embedder's, never lb's.** Which tags a deployment's table shows as columns
//! (a site, a region, a subsystem) is product vocabulary, so lb names none: the node is handed a
//! list at boot (`BootConfig::insight_search_tags`) and the host copies it onto the filter
//! (`ListFilter::search_tags`, never read from the wire). An empty list is the shipped behaviour:
//! the title alone.
//!
//! **One full-text index per key.** A search is `title @@ q OR tags.k1 @@ q OR …`, and every branch
//! of that `OR` must be answered from an index or the `@@` has nothing to match against. The
//! indexes are defined with the title's analyzer, so a prefix finds a tag value exactly as it finds a
//! word of the title. `DEFINE INDEX` builds over the rows already stored, so a key added to the
//! config is searchable once its index is built, with no backfill.
//!
//! **The CALLER builds them, once** ([`ensure_search_tag_indexes`]), before a search that names the
//! keys: `list` does not. Building ten indexes inside every search made concurrent first searches
//! conflict and fail (the host's `insight/search_schema.rs` has the measurement and the fix).
//!
//! One responsibility: validate the configured keys and ensure their indexes.

use lb_store::{Store, StoreError};

use crate::error::InsightsError;
use crate::insight::OCC_TABLE;
use crate::schema::ANALYZER;

/// The most keys one node may search. Each key is a real BM25 index (prefix indexing is about 2.5×
/// the plain size, `insight-indexes-scope.md`) and one more `OR` branch on every search, so the list
/// is a handful of display columns, not every tag a producer writes.
pub const MAX_SEARCH_TAGS: usize = 16;

/// Refuse a key list the search could not use safely. Called at boot, so a typo in the config stops
/// the node with a message rather than silently searching less than the operator configured.
///
/// A key is a plain lowercase identifier because it is spliced into SQL as a field name
/// (`data.tags.<key>`) and into an index name; the same rule the entity limit's key follows.
pub fn validate_search_tags(keys: &[String]) -> Result<(), InsightsError> {
    if keys.len() > MAX_SEARCH_TAGS {
        return Err(InsightsError::BadInput(format!(
            "insight search tags: at most {MAX_SEARCH_TAGS} keys, got {}",
            keys.len()
        )));
    }
    for (i, key) in keys.iter().enumerate() {
        if !plain_ident(key) {
            return Err(InsightsError::BadInput(format!(
                "insight search tag {key:?} must be a lowercase identifier (a-z, 0-9, _)"
            )));
        }
        if keys[..i].contains(key) {
            return Err(InsightsError::BadInput(format!(
                "insight search tag {key:?} is listed twice"
            )));
        }
    }
    Ok(())
}

/// Define the full-text index for each key in `ws`. Idempotent (`IF NOT EXISTS`), so it is safe to
/// retry; slow on the first run (it builds over every stored row), so call it once per workspace, not
/// per request.
///
/// A key that is not a plain identifier is skipped here as well as refused at boot, so no path can
/// splice one into DDL.
pub async fn ensure_search_tag_indexes(
    store: &Store,
    ws: &str,
    keys: &[String],
) -> Result<(), StoreError> {
    let ddl: String = keys
        .iter()
        .filter(|k| plain_ident(k))
        .map(|k| {
            format!(
                "DEFINE INDEX IF NOT EXISTS {} ON TABLE {OCC_TABLE} \
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
    format!("insight_tag_{key}_v2")
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

    fn keys(ks: &[&str]) -> Vec<String> {
        ks.iter().map(|k| k.to_string()).collect()
    }

    #[test]
    fn plain_keys_pass() {
        assert!(validate_search_tags(&keys(&["site", "short_name", "zone2"])).is_ok());
        assert!(validate_search_tags(&[]).is_ok());
    }

    #[test]
    fn a_key_that_could_break_the_sql_is_refused() {
        for bad in ["Site", "site name", "tags.site", "site;DROP", "", "2site"] {
            assert!(
                validate_search_tags(&keys(&[bad])).is_err(),
                "{bad:?} must be refused"
            );
        }
    }

    #[test]
    fn a_duplicate_and_an_oversized_list_are_refused() {
        assert!(validate_search_tags(&keys(&["site", "site"])).is_err());
        let many: Vec<String> = (0..=MAX_SEARCH_TAGS).map(|i| format!("k{i}")).collect();
        assert!(validate_search_tags(&many).is_err());
    }
}
