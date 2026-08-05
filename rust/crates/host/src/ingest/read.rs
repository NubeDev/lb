//! `series.read` / `series.latest` — authorize, then read the committed series. Both are
//! namespace-scoped through the store, so a ws-B reader can physically only see ws-B's series
//! (the hard wall) and a denied caller learns nothing (ingest scope).
//!
//! `series.read` has three shapes under ONE cap (`mcp:series.read:call`), all re-authorized per
//! call — a cursor is a bookmark, never a grant:
//!   - the legacy raw range ([`series_read_range`], kept for internal callers);
//!   - the keyset **page** ([`series_read_page`]) — `{limit, cursor, direction}` + seq/time bounds
//!     (series-paging scope, slice B);
//!   - the **bucketed** decimation ([`series_read_buckets`]) — `{t, min, max, avg, last, count}`
//!     per bucket (series-decimation scope, slice C).

use lb_auth::Principal;
use lb_ingest::{
    apply_method, latest as series_latest, latest_many as series_latest_many_read, list_policies,
    read as series_read, read_buckets, read_page, read_rollups, resolve_policy, Align, Bucket,
    BucketQuery, Method, Page, PageError, PageQuery, RollupRow, Sample,
};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Range query over the committed `series` in `ws`: samples with `seq` in `[from_seq, to_seq]`,
/// ordered by `seq`. Gated by `mcp:series.read:call`.
pub async fn series_read_range(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    from_seq: Option<u64>,
    to_seq: Option<u64>,
) -> Result<Vec<Sample>, IngestError> {
    authorize_ingest(principal, ws, "series.read")?;
    Ok(series_read(store, ws, series, from_seq, to_seq).await?)
}

/// One keyset page of `series` in `ws`. Re-authorizes `mcp:series.read:call` on EVERY page — a
/// revoked grant denies the next page even with a valid cursor.
pub async fn series_read_page(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    q: &PageQuery,
) -> Result<Page, IngestError> {
    authorize_ingest(principal, ws, "series.read")?;
    read_page(store, ws, series, q).await.map_err(page_err)
}

/// The STORED rollup rows of `series` in `[from_ts, to_ts)` — `series_rollup` verbatim, no merge.
///
/// **Why this exists next to the bucketed read.** `series_read_buckets` answers "what happened over
/// this window", merging the stored tail under live raw and returning one flat shape. That is the
/// right answer for a chart, and the wrong one for two questions an operator actually has: *did the
/// GC fold anything*, and *what exactly is on disc for this tier*. Those cannot be asked of the
/// merged read at all — [`merge_rollups`](lb_ingest::read_buckets) SUPPRESSES a stored row while the
/// raw beneath it survives, so a window can hold rollup rows and report none of them.
///
/// So this is deliberately NOT a fallback path and never merges: it returns stored rows or an empty
/// vec. An empty result means "no rows on disc in this window", which is a real, actionable answer —
/// quietly substituting decimated raw would destroy the only signal the verb exists to carry.
///
/// Rows come back at their own `width_ms`, on the tier's own grid, carrying the full stat set
/// (`min`/`max`/`sum`/`num_count`/`count`/`last`/`first`) — `sum`+`count` rather than a mean, so a
/// caller re-aggregating into a wider bucket stays exact. Gated by the same `mcp:series.read:call`
/// as every other projection of this series: same data, same wall, no new grammar.
pub async fn series_read_rollups(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    from_ts: u64,
    to_ts: u64,
) -> Result<Vec<RollupRow>, IngestError> {
    authorize_ingest(principal, ws, "series.read")?;
    // A fully-qualified series in an equality predicate — `read_rollups` binds `$series` exactly,
    // never a prefix scan. Load-bearing: a prefix scan over a device's full history blew the
    // store's ~5s query timeout at 62,663 rows.
    Ok(read_rollups(store, ws, series, from_ts, to_ts).await?)
}

/// Bucketed decimation of `series` over a wall-clock window. Same cap as the row read.
///
/// Returns `(buckets, method, align)` — the data plus BOTH resolved read parameters, so a caller
/// never has to guess which method produced the `value` column it is charting, or which grid the
/// `t` values it is plotting fall on.
///
/// Both are resolved from the SAME governing policy — `resolve_policy`'s longest prefix match, the
/// one the GC and the commit filter use, so a state series on its own longer prefix reads as `last`
/// while its analog neighbours ride the parent's `avg`.
///
/// - **Method precedence:** the caller's explicit `override_method` wins; else the policy's
///   [`Policy::method_for`]. Neither → `None`: the full stat row, no `value`, as before.
/// - **Alignment precedence:** an explicit `q.align` wins; else the policy's [`Policy::align_for`].
///   Neither → `None`: the UTC epoch grid, as before.
///
/// **Resolving the alignment here is not a convenience — it is the correctness seam.** The GC folded
/// each tier on that tier's own grid; a read that floored on the epoch grid instead would merge
/// those stored rows into buckets whose boundaries it invented, mixing two griddings with nothing
/// raised. Read and fold agree because both ask the policy (`series_align_grid_test` folds through a
/// real GC pass and reads back through both read paths to hold it).
pub async fn series_read_buckets(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    q: &BucketQuery,
    width_ms: u64,
    override_method: Option<Method>,
) -> Result<(Vec<Bucket>, Option<Method>, Option<Align>), IngestError> {
    authorize_ingest(principal, ws, "series.read")?;

    // ONE policy read serves both axes. Skipped entirely when the caller has pinned both — the read
    // then needs nothing from the policy, and this is the hot dashboard path.
    let governing = if q.align.is_none() || override_method.is_none() {
        // `resolve_policy` — the LONGEST matching prefix, the same rule the GC and the commit filter
        // use. Cloned because the list is dropped here; a policy is a handful of small fields.
        resolve_policy(&list_policies(store, ws).await?, series).cloned()
    } else {
        None
    };

    let mut effective = q.clone();
    if effective.align.is_none() {
        effective.align = governing.as_ref().and_then(|p| p.align_for(width_ms));
    }
    let mut buckets = read_buckets(store, ws, series, &effective, width_ms)
        .await
        .map_err(page_err)?;

    let method = match override_method {
        Some(m) => Some(m),
        None => governing.as_ref().and_then(|p| p.method_for(width_ms)),
    };
    if let Some(m) = method {
        // A method whose representative this tier never stored is a `BadInput` naming the fix —
        // never a silently-approximated neighbour (series-normalize open question, decided).
        apply_method(&mut buckets, m).map_err(IngestError::BadInput)?;
    }
    Ok((buckets, method, effective.align))
}

fn page_err(e: PageError) -> IngestError {
    match e {
        PageError::BadCursor(m) => IngestError::BadInput(m),
        PageError::Store(s) => IngestError::Store(s),
    }
}

/// The newest committed sample of `series` in `ws` (highest `seq`), or `None`. Gated by
/// `mcp:series.latest:call`. Generic "last value" — not a device shadow.
pub async fn series_latest_value(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
) -> Result<Option<Sample>, IngestError> {
    authorize_ingest(principal, ws, "series.latest")?;
    Ok(series_latest(store, ws, series).await?)
}

/// The newest committed sample of EACH named series in `ws`, in one round-trip (series-read-perf
/// scope). Authorizes `mcp:series.latest:call` **ONCE for the whole batch** — the batch is one
/// logical read of the series-latest surface, not K grants; a principal without the grant is denied
/// the entire batch (it cannot read a latest here it could not read singly). Every requested name
/// appears in the result (absent series → `None`), workspace-first so a ws-B caller sees only ws-B.
pub async fn series_latest_many(
    store: &Store,
    principal: &Principal,
    ws: &str,
    names: &[String],
) -> Result<Vec<(String, Option<Sample>)>, IngestError> {
    authorize_ingest(principal, ws, "series.latest")?;
    Ok(series_latest_many_read(store, ws, names).await?)
}
