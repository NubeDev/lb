//! **Test-only** seed routes for the UI's real-gateway Vitest harness (`test_gateway` bin). They let
//! a Node test process put **real records** into the real node's store for surfaces that have no
//! public *create* route (an inbox item, an outbox effect, an extension install) — so a UI test reads
//! them back over the real read routes. This is **seeding, not faking** (testing-scope §3.1): each
//! `/_seed/*` route calls the real host/crate write verb, writing into the workspace namespace the
//! caller's token binds — the exact path production data flows through, behind the same workspace wall.
//!
//! These routes exist ONLY in the test gateway binary; the production gateway (`router`) never mounts
//! them. They are authenticated like every other route (the workspace comes from the token, §7), so a
//! seed cannot cross the workspace wall.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use lb_assets::{record_install, ExtUi, Install, Tier};
use lb_host::{Provenance, Qos, Sample, Tag, TagSource};
use lb_inbox::Item;
use lb_outbox::{enqueue, Effect};
use lb_role_gateway::{authenticate, Gateway};
use serde::Deserialize;
use serde_json::Value;

/// Mount the `/_seed/*` routes onto a router (test gateway only).
pub fn seed_routes(router: Router<Gateway>) -> Router<Gateway> {
    router
        .route("/_seed/inbox", post(seed_inbox))
        .route("/_seed/outbox", post(seed_outbox))
        .route("/_seed/extension", post(seed_extension))
        .route("/_seed/series", post(seed_series))
}

fn auth(gw: &Gateway, headers: &HeaderMap) -> Result<lb_auth::Principal, (StatusCode, String)> {
    authenticate(gw, headers).map_err(|_| (StatusCode::UNAUTHORIZED, "bad token".into()))
}

/// `POST /_seed/inbox` — write a real durable inbox `Item` into the token's workspace.
async fn seed_inbox(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(item): Json<Item>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = auth(&gw, &headers)?;
    lb_inbox::record(&gw.node.store, p.ws(), &item)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /_seed/outbox` body — the effect to enqueue (+ the change row it tracks, kept minimal).
#[derive(Deserialize)]
struct SeedEffect {
    effect: Effect,
}

/// `POST /_seed/outbox` — enqueue a real outbox `Effect` into the token's workspace (the same write
/// the workflow's `start_job` performs, minus the workflow).
async fn seed_outbox(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SeedEffect>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = auth(&gw, &headers)?;
    // The effect tracks a synthetic change row (the relay never reads it for a seeded effect).
    let change = serde_json::json!({ "seeded": true });
    enqueue(
        &gw.node.store,
        p.ws(),
        "seed_change",
        &body.effect.id,
        &change,
        &body.effect,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /_seed/extension` body — a minimal install descriptor (the UI reads `ext.list`).
#[derive(Deserialize)]
struct SeedExt {
    ext: String,
    version: String,
    #[serde(default)]
    tier: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    ui: Option<ExtUi>,
    /// The widget tiles this install contributes (dashboard-widgets scope) — one per `[[widget]]`.
    #[serde(default)]
    widgets: Vec<ExtUi>,
}

/// `POST /_seed/extension` — write a real `Install` record into the token's workspace, so the
/// Extensions console + the ext-host page read it back over `ext.list`.
async fn seed_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SeedExt>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = auth(&gw, &headers)?;
    let tier = match body.tier.as_deref() {
        Some("native") => Tier::Native,
        _ => Tier::Wasm,
    };
    let mut install = Install::new(body.ext, body.version, Vec::<String>::new(), 0);
    install.tier = tier;
    install.enabled = body.enabled.unwrap_or(true);
    install.ui = body.ui;
    install.widgets = body.widgets;
    record_install(&gw.node.store, p.ws(), &install)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /_seed/series` body — one discoverable series: a committed sample value + the tag facet that
/// makes it findable. `key:value` is the facet the proof-panel page searches by; `payload` is what
/// `series.latest` returns.
#[derive(Deserialize)]
struct SeedSeries {
    series: String,
    seq: u64,
    payload: Value,
    key: String,
    value: Value,
}

/// `POST /_seed/series` — seed ONE discoverable series through the REAL write paths (not a fake):
///   1. `ingest_write` + `drain_workspace` commit the sample, so `series.latest` reads its value;
///   2. `lb_tags::add` applies a `key:value` edge on the `series:<name>` entity, so `series.find`
///      (tag-graph intersection) discovers it.
/// Step 2 is explicit because the ingest path does not convert a sample's `labels` into tag edges
/// today (see debugging/extensions/series-find-needs-tag-edges-not-labels.md) — this seeds the edge a
/// producer's labels *should* eventually produce, behind the same workspace wall as production data.
async fn seed_series(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SeedSeries>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = auth(&gw, &headers)?;
    let ws = p.ws();
    let sample = Sample {
        series: body.series.clone(),
        producer: String::new(),
        ts: body.seq,
        seq: body.seq,
        payload: body.payload,
        labels: Value::Null,
        qos: Qos::BestEffort,
    };
    lb_host::ingest_write(&gw.node.store, &p, ws, vec![sample])
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    lb_host::drain_workspace(&gw.node.store, ws)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let tag = Tag::new(body.key, body.value);
    let prov = Provenance::new(body.seq, p.sub().to_string(), TagSource::Producer);
    lb_host::tags_add(
        &gw.node.store,
        &p,
        ws,
        &format!("series:{}", body.series),
        &tag,
        &prov,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("tag: {e:?}")))?;
    Ok(StatusCode::NO_CONTENT)
}
