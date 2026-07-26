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
    read as series_read, read_buckets, read_page, resolve_policy, Bucket, BucketQuery, Method,
    Page, PageError, PageQuery, Sample,
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

/// Bucketed decimation of `series` over a wall-clock window. Same cap as the row read.
///
/// Returns `(buckets, method)`. When a `method` governs the read, every bucket carries its `value`
/// column and the resolved method is reported back so the caller never has to guess which one it
/// got (series-normalize scope).
///
/// Method precedence: the caller's explicit `override_method` wins; otherwise the retention tier at
/// exactly this width, under the policy whose prefix is the LONGEST match for `series` — the same
/// `resolve_policy` the GC and the commit filter use, so a state series on its own longer prefix
/// reads as `last` while its analog neighbours ride the parent's `avg`. No policy, no tier at this
/// width, and no override → `None`, which is today's exact behaviour: the full stat row, no `value`.
pub async fn series_read_buckets(
    store: &Store,
    principal: &Principal,
    ws: &str,
    series: &str,
    q: &BucketQuery,
    width_ms: u64,
    override_method: Option<Method>,
) -> Result<(Vec<Bucket>, Option<Method>), IngestError> {
    authorize_ingest(principal, ws, "series.read")?;
    let mut buckets = read_buckets(store, ws, series, q, width_ms)
        .await
        .map_err(page_err)?;

    let method = match override_method {
        Some(m) => Some(m),
        None => resolve_policy(&list_policies(store, ws).await?, series)
            .and_then(|p| p.tier_at(width_ms))
            .and_then(|t| t.method),
    };
    if let Some(m) = method {
        // A method whose representative this tier never stored is a `BadInput` naming the fix —
        // never a silently-approximated neighbour (series-normalize open question, decided).
        apply_method(&mut buckets, m).map_err(IngestError::BadInput)?;
    }
    Ok((buckets, method))
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
