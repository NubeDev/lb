//! Ingest routes — the browser's `ingest.*` / `series.*` surface over the gateway (data-console
//! scope). The host verbs shipped in S8 but were never reachable over the gateway; this exposes them
//! for the Ingest page: list/search series, read latest + recent samples, and push one sample by hand.
//! Each route mirrors a `lb_host::<verb>` 1:1 and re-runs the host's gate server-side (workspace-first,
//! then `mcp:<verb>:call`). The workspace + producer come from the **token**, never the body (§7) — so
//! a written sample's producer is the authenticated principal, un-spoofable.
//!
//! **Write-then-read visibility:** `ingest.write` durable-appends to staging (the cheap path); the
//! committed `series` table is what `series.read`/`latest`/`list` read. So the write route **drains
//! this workspace** after staging — the gateway node carries the ingest path, so the manual sample is
//! committed and visible on the very next read (the UI refresh shows it). The drain is idempotent
//! (exactly-once per `(series, producer, seq)`), so this never double-commits.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{own_batches, IngestError, Sample};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `POST /ingest` body — the samples to push. The producer field is ignored/overwritten with the
/// token's principal by the host verb (un-spoofable).
#[derive(Debug, Deserialize)]
pub struct WriteSamples {
    pub samples: Vec<Sample>,
}

/// `POST /ingest` — stage `samples` as the token's principal, then drain this workspace so the
/// sample is committed and immediately visible to the reads. Returns `{ accepted, committed }`.
pub async fn write_samples(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<WriteSamples>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    // Keep a stamped copy to publish as live motion after the durable write — the producer is the
    // authenticated principal (matching what `ingest_write` stamps), so a live frame is consistent
    // with the committed `series` row.
    let live: Vec<Sample> = body
        .samples
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.producer = p.sub().to_string();
            s
        })
        .collect();
    let accepted = lb_host::ingest_write(&gw.node.store, &p, p.ws(), body.samples)
        .await
        .map_err(ingest_status)?;
    // Commit staging so the just-written sample is visible to the next read (the UI refresh). The
    // gateway node carries the ingest path; the drain is exactly-once.
    //
    // BOUNDED to this request's own work (drain-backpressure scope): an unbounded drain here billed
    // the POSTing producer for the whole workspace's staged backlog — O(backlog) on a request, which
    // measured 18.5s for a single sample behind a 4,671-row backlog. The ingest reactor owns the
    // remainder. `committed` therefore reports what THIS request committed, not the workspace total.
    let pass = lb_host::drain_workspace_bounded(&gw.node.store, p.ws(), own_batches(accepted))
        .await
        .map_err(ingest_status)?;
    // Publish each committed sample onto its series motion subject so a live dashboard widget sees
    // it advance without polling (state vs motion, rule 3). Best-effort — a publish failure never
    // fails the durable write the read path already reflects.
    for s in &live {
        let _ = lb_host::publish_sample(&gw.node.bus, p.ws(), s).await;
    }
    Ok(Json(
        json!({ "accepted": accepted, "committed": pass.committed }),
    ))
}

/// `GET /series?prefix=` query — list series names by prefix (the discovery list).
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub prefix: Option<String>,
}

/// `GET /series` — list the workspace's series names (optionally by prefix). Gated `series.list`.
pub async fn list_series(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let names = lb_host::series_list(
        &gw.node.store,
        &p,
        p.ws(),
        q.prefix.as_deref().unwrap_or(""),
    )
    .await
    .map_err(ingest_status)?;
    Ok(Json(json!({ "series": names })))
}

/// `POST /series/find` body — the tag facets to intersect (the faceted search). A facet is
/// `{ key, value? }`: value present → exact, absent → key-only.
#[derive(Debug, Deserialize)]
pub struct FindFacets {
    pub facets: Vec<FacetArg>,
}

#[derive(Debug, Deserialize)]
pub struct FacetArg {
    pub key: String,
    #[serde(default)]
    pub value: Option<Value>,
}

/// `POST /series/find` — find series whose entity carries ALL facets (tag-graph intersection). Gated
/// `series.find`. POST (not GET) because the facet set is a structured body.
pub async fn find_series(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<FindFacets>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let facets: Vec<lb_host::Facet> = body
        .facets
        .into_iter()
        .map(|f| match f.value {
            Some(v) => lb_host::Facet::exact(f.key, v),
            None => lb_host::Facet::key_only(f.key),
        })
        .collect();
    let hits = lb_host::series_find(&gw.node.store, &p, p.ws(), &facets)
        .await
        .map_err(ingest_status)?;
    Ok(Json(json!({ "series": hits })))
}

/// `GET /series/{series}/latest` — the newest committed sample (or `null`). Gated `series.latest`.
pub async fn latest_sample(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(series): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let last = lb_host::series_latest_value(&gw.node.store, &p, p.ws(), &series)
        .await
        .map_err(ingest_status)?;
    Ok(Json(json!({ "sample": last })))
}

/// `GET /series/{series}/samples?from=&to=` query — a bounded range of recent samples.
///
/// `mode=buckets` switches to the decimated read (see [`read_samples`]). The bucket params ride the
/// same struct because Axum rejects nothing it does not recognise: an unknown query param is
/// silently DROPPED, so a caller asking for `mode=buckets` against a struct without the field got a
/// raw read and an empty `{"samples":[]}` — data that exists, reported as absent, with no error.
/// That is exactly the silent-degrade this crate's ingest work exists to end, so the fields are
/// declared here even though only `mode` selects between the two paths.
#[derive(Debug, Deserialize)]
pub struct ReadQuery {
    pub from: Option<u64>,
    pub to: Option<u64>,
    /// `"buckets"` for the decimated read; absent/anything else is the raw range read.
    pub mode: Option<String>,
    pub width_ms: Option<u64>,
    pub budget: Option<usize>,
    /// Signed — a west-of-Greenwich grid anchor is ordinary.
    pub origin_ms: Option<i64>,
    pub method: Option<String>,
}

/// `GET /series/{series}/samples` — committed samples in `[from, to]` ordered by seq, or, with
/// `mode=buckets`, the server-side decimated read. Gated `series.read` either way.
///
/// **Why buckets belong on this route.** The raw table holds only the retention window (30 min on a
/// stock modbus node); everything older lives as rolled-up tier rows. A raw-only read therefore goes
/// EMPTY the moment the GC runs, which reads as "no data" on a series with months of history. The
/// bucketed path merges the rolled-up tail (`bucket::merge_rollups`) with the live raw window in one
/// read, so a caller gets a continuous line across the raw→rollup boundary instead of a cliff.
/// The MCP verb (`series.read {mode:"buckets"}`) has always been able to do this; the HTTP route
/// could not, which is why the UI could not show long-term history.
pub async fn read_samples(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(series): Path<String>,
    Query(q): Query<ReadQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;

    if q.mode.as_deref() == Some("buckets") {
        return read_buckets(&gw, &p, &series, &q).await;
    }

    let rows = lb_host::series_read_range(&gw.node.store, &p, p.ws(), &series, q.from, q.to)
        .await
        .map_err(ingest_status)?;
    Ok(Json(json!({ "samples": rows })))
}

/// The `mode=buckets` half. A 1:1 mirror of the `series.read {mode:"buckets"}` MCP verb — same
/// query shape, same width resolution, same reply keys — so the two surfaces cannot drift into
/// disagreeing about what a bucket is. A missing bound is a `400` naming the field, never a silent
/// fallback to the raw read: an empty chart that should have been an error is the failure this whole
/// path is meant to prevent.
async fn read_buckets(
    gw: &Gateway,
    p: &lb_auth::Principal,
    series: &str,
    q: &ReadQuery,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Generic over `Display` so a literal and a `String` error from the ingest crate both land here
    // verbatim — the caller is told which field was wrong, never a generic 400.
    let bad = |m: &dyn std::fmt::Display| (StatusCode::BAD_REQUEST, m.to_string());

    let query = lb_host::BucketQuery {
        from_ts: q
            .from
            .ok_or_else(|| bad(&"buckets mode needs from (epoch ms)"))?,
        to_ts: q
            .to
            .ok_or_else(|| bad(&"buckets mode needs to (epoch ms)"))?,
        width_ms: q.width_ms,
        budget: q.budget,
        align: q.origin_ms.map(|origin_ms| lb_host::Align { origin_ms }),
    };
    let width = lb_host::effective_width(&query).map_err(|e| bad(&e))?;
    let method = match q.method.as_deref() {
        Some(name) => Some(lb_host::Method::parse(name).map_err(|e| bad(&e))?),
        None => None,
    };

    let (buckets, resolved, align) =
        lb_host::series_read_buckets(&gw.node.store, p, p.ws(), series, &query, width, method)
            .await
            .map_err(ingest_status)?;

    Ok(Json(json!({
        "buckets": buckets,
        "width_ms": width,
        "method": resolved.map(|m| m.as_str()),
        "origin_ms": align.map(|a| a.origin_ms),
    })))
}

/// `DELETE /series/{series}/samples` body — the selector: explicit `keys` ([{producer, seq}]) XOR
/// a `from`/`to` seq range (at least one bound). Neither → `400` (a whole-series delete must be the
/// explicit `DELETE /series/{series}`).
#[derive(Debug, Deserialize)]
pub struct DeleteSamplesBody {
    #[serde(default)]
    pub keys: Option<Vec<lb_host::SampleKey>>,
    #[serde(default)]
    pub from: Option<u64>,
    #[serde(default)]
    pub to: Option<u64>,
}

/// `DELETE /series/{series}/samples` — bulk-delete committed raw samples. Gated
/// `series.samples.delete` (admin-only). Rolled-up history is untouched. Returns `{ deleted }` —
/// the rows actually removed. The workspace comes from the token, never the body.
pub async fn delete_series_samples_route(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(series): Path<String>,
    Json(body): Json<DeleteSamplesBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let deleted = lb_host::series_samples_delete(
        &gw.node.store,
        &p,
        p.ws(),
        &series,
        body.keys,
        body.from,
        body.to,
    )
    .await
    .map_err(ingest_status)?;
    Ok(Json(json!({ "deleted": deleted })))
}

/// `PATCH /series/{series}/samples` body — the in-place edits. Each entry names an existing
/// sample by `(producer, seq)` and must set at least one of `payload`/`ts` (epoch ms).
#[derive(Debug, Deserialize)]
pub struct UpdateSamplesBody {
    pub updates: Vec<lb_host::SampleUpdate>,
}

/// `PATCH /series/{series}/samples` — edit committed raw samples in place. Gated
/// `series.samples.update` (admin-only). UPDATE semantics, never UPSERT: a missing sample is
/// skipped, never created. Returns `{ updated }`. The workspace comes from the token.
pub async fn update_series_samples_route(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(series): Path<String>,
    Json(body): Json<UpdateSamplesBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let updated = lb_host::series_samples_update(&gw.node.store, &p, p.ws(), &series, body.updates)
        .await
        .map_err(ingest_status)?;
    Ok(Json(json!({ "updated": updated })))
}

/// `DELETE /series/{series}` — remove the series and its whole footprint (samples, rollups, staged
/// rows, registry row, tag edges). Gated `series.delete`. Idempotent — deleting an unknown series
/// succeeds. The workspace comes from the token (the hard wall), never the path.
pub async fn delete_series_route(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(series): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::series_delete(&gw.node.store, &p, p.ws(), &series)
        .await
        .map_err(ingest_status)?;
    Ok(Json(json!({ "ok": true })))
}

/// `POST /series/{series}/rename` body — the new name. Rejected (`400`) if it is already in use.
#[derive(Debug, Deserialize)]
pub struct RenameBody {
    pub to: String,
}

/// `POST /series/{series}/rename` — rename `{series}` → `to`, carrying its whole footprint. Gated
/// `series.rename`. Refuses (`400`) a target that already exists (no silent merge). The workspace
/// comes from the token.
pub async fn rename_series_route(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(series): Path<String>,
    Json(body): Json<RenameBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::series_rename(&gw.node.store, &p, p.ws(), &series, &body.to)
        .await
        .map_err(ingest_status)?;
    Ok(Json(json!({ "ok": true })))
}

/// Map an ingest gate outcome onto an HTTP status. `Denied` is `403` (opaque); `BadInput` is `400`;
/// a store fault is `403`-opaque like the other gateway routes.
fn ingest_status(e: IngestError) -> (StatusCode, String) {
    match e {
        IngestError::Denied => (StatusCode::FORBIDDEN, e.to_string()),
        IngestError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        IngestError::Store(s) => (StatusCode::FORBIDDEN, s.to_string()),
    }
}
