//! The insight table's indexes — defined once per workspace, idempotently (insights umbrella scope).
//!
//! Two indexes, and deliberately only two. Each is here because a MEASURED query needed it:
//!
//! 1. **`dedup_key`** — the raise path looks an insight up by its stable identity on EVERY raise
//!    (`insight_id::dedup_lookup`). Without an index that is a full table scan: SurrealDB's own
//!    `EXPLAIN` reported `TableScan`, and because insight documents exceed the 1 KB value threshold
//!    they live in the value log, so the scan reads every document off disc to answer one equality.
//!    Measured on the shipped ESR data: 197 documents, ~594 KB, on every raise, and rules fire every
//!    15 minutes. With the index the plan becomes `IndexScan`.
//!
//! 2. **`title`, full text** — the roster's search box. `DEFINE ANALYZER` + BM25, the same shape
//!    `lb_tags::define_text_index` uses. Note the keyword is `FULLTEXT`, not the `SEARCH` of
//!    SurrealDB 2.x — on 3.x the old spelling is rejected by the parser outright. NAME ONLY, by
//!    decision: searching the tag-derived
//!    columns too would mean indexing several more fields for a box most people type a fault name
//!    into.
//!
//!    **`edgengram(2,15)` is what makes a PREFIX match.** Measured: with `lowercase` alone the
//!    analyzer indexes whole words, so `hi` does not find "High Daily Usage" — which is how a
//!    search box is actually used. With edgengram it does, while whole words still match.
//!    `blank` is kept as the only tokenizer on evidence: adding `class` splits letters from digits
//!    and BREAKS `msb1` (it becomes `msb` + `1`), while `blank` alone already matches `mdb` inside
//!    `MDB-1-4`, because the prefix filter runs over the whole hyphenated token.
//!
//! **Why there is no index on `last_ts`, `status` or `severity`.** Measured, not assumed:
//!
//! - `status`/`severity` are three-value fields matching 39–64% of rows. A scan beats an index at
//!   that selectivity, and an index would be maintenance cost for a worse plan.
//! - `last_ts` cannot give ordered pagination in SurrealDB 3.2.4. `ORDER BY` keeps a
//!   `SortTopKByKey` step in EVERY plan shape tested — with the index, with an always-true bound,
//!   with the `WITH INDEX` hint (which the planner ignores), and with the compound keyset predicate.
//!   A descending index is rejected outright by the parser. The cost therefore tracks the rows the
//!   predicate MATCHES, not `LIMIT`: at 20,000 rows a page near the newest end took 173 ms and one
//!   near the oldest end 1.6 ms — the same query shape, 100× apart. Adding the index made page 1
//!   *slower* (47 ms vs 28 ms at 5,000 rows) because it adds indirection and removes no work.
//!
//! So ordering stays a sort over the matched set, and the honest lever is narrowing that set.

use lb_store::{Store, StoreError};

use crate::insight::OCC_TABLE;

/// The analyzer backing the title search, and the index that uses it.
///
/// **Both names carry a version suffix, and that is load-bearing.** The DDL below is
/// `IF NOT EXISTS`, so a workspace that already defined `insight_text_v1` would keep it forever and
/// never see a changed definition — the statement is a no-op once the name exists. Bumping the
/// suffix makes a changed analyzer a genuinely NEW object, so it is created on the next raise. The
/// alternative, `OVERWRITE`, would rebuild the whole index on every single raise.
///
/// **A bump alone is NOT enough, and this was found the hard way.** With two full-text indexes on
/// the same field the planner picks the OLDER one: after defining `insight_name_v2`, a search for
/// `hi` still ran against `insight_name` and returned nothing, while `high` matched — the v1
/// whole-word index was answering. So the superseded index is REMOVED here before the new one is
/// defined. `IF EXISTS`, so it is a no-op once done.
///
/// Removing an index a reader might be mid-query on is not free, but the alternative is worse: an
/// upgraded node that silently keeps answering from the stale index forever.
/// Named distinctly from the INDEX (`insight_name`): they share a namespace in the reader's head
/// even if not in the engine, and one name for two things is how a DDL edit goes wrong later.
const ANALYZER: &str = "insight_text_v2";
/// The index name, versioned with the analyzer — a new analyzer needs a new index to use it.
const NAME_INDEX: &str = "insight_name_v2";
/// The index this one replaces. Removed on the next raise/search so the planner cannot keep
/// answering from it — see the note above.
const SUPERSEDED_INDEX: &str = "insight_name";

/// Define the insight indexes for `ws`. Idempotent (`IF NOT EXISTS`), so it is safe to call on every
/// write — the pattern `ensure_series_schema` uses, for the same reason: there is no boot-time hook
/// that knows which workspaces exist.
pub async fn ensure_insight_schema(store: &Store, ws: &str) -> Result<(), StoreError> {
    store
        .query_ws(
            ws,
            &format!(
                "REMOVE INDEX IF EXISTS {SUPERSEDED_INDEX} ON TABLE {OCC_TABLE};
                 DEFINE ANALYZER IF NOT EXISTS {ANALYZER} \
                    TOKENIZERS blank FILTERS lowercase,edgengram(2,15);
                 DEFINE INDEX IF NOT EXISTS insight_dedup ON TABLE {OCC_TABLE} FIELDS data.dedup_key;
                 DEFINE INDEX IF NOT EXISTS {NAME_INDEX} ON TABLE {OCC_TABLE} \
                    FIELDS data.title FULLTEXT ANALYZER {ANALYZER} BM25;"
            ),
            vec![],
        )
        .await?;
    Ok(())
}
