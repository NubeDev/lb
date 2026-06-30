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
use lb_auth::{mint, Claims, Role};
use lb_flows::NodeBlock;
use lb_host::{Provenance, Qos, Sample, Tag, TagSource};
use lb_inbox::Item;
use lb_outbox::{enqueue, Effect};
use lb_role_gateway::{authenticate, Gateway};
use lb_telemetry::TelemetryRecord;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

/// Mount the `/_seed/*` routes onto a router (test gateway only).
pub fn seed_routes(router: Router<Gateway>) -> Router<Gateway> {
    router
        .route("/_seed/inbox", post(seed_inbox))
        .route("/_seed/outbox", post(seed_outbox))
        .route("/_seed/extension", post(seed_extension))
        .route("/_seed/iot_demo", post(seed_iot_demo))
        .route("/_seed/series", post(seed_series))
        .route("/_seed/proof_panel", post(seed_proof_panel))
        .route("/_seed/session", post(seed_session))
        .route("/_seed/flow_node", post(seed_flow_node))
        .route("/_seed/telemetry", post(seed_telemetry))
}

/// `POST /_seed/telemetry` body — one telemetry row to plant into the token's workspace ring. All
/// fields optional with sensible defaults; `params_digest` is passed already-redacted (a seed never
/// carries a raw secret). `cap` bounds the ring (defaults to the production per-source cap).
#[derive(Debug, Deserialize)]
struct SeedTelemetry {
    #[serde(default = "default_level")]
    level: String,
    #[serde(default = "default_actor")]
    actor: String,
    #[serde(default)]
    tool: String,
    #[serde(default = "default_source")]
    source: String,
    #[serde(default)]
    trace_id: String,
    #[serde(default = "default_outcome")]
    outcome: String,
    #[serde(default)]
    ts: u64,
    #[serde(default)]
    msg: String,
    #[serde(default)]
    params_digest: String,
    #[serde(default)]
    fields: Value,
    #[serde(default = "default_cap")]
    cap: usize,
}

fn default_level() -> String {
    "info".into()
}
fn default_actor() -> String {
    "user:ada".into()
}
fn default_source() -> String {
    "host".into()
}
fn default_outcome() -> String {
    "allow".into()
}
fn default_cap() -> usize {
    1000
}

/// `POST /_seed/telemetry` — write a real telemetry row into the token's workspace capped ring AND
/// mirror it onto the ws-walled tail subject, through `lb_host::telemetry_seed` (the same
/// `capped_insert` + tail publish the `SurrealCappedLayer` performs on a live event). Seeding, not
/// faking: the console reads it back over the real `telemetry.query`/`tail` path, behind the same
/// workspace wall. Returns the row's `seq`.
async fn seed_telemetry(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SeedTelemetry>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = auth(&gw, &headers).await?;
    let record = TelemetryRecord {
        level: body.level,
        ws: p.ws().to_string(),
        actor: body.actor,
        tool: body.tool,
        source: body.source,
        trace_id: body.trace_id,
        outcome: body.outcome,
        ts: body.ts,
        msg: body.msg,
        params_digest: body.params_digest,
        fields: if body.fields.is_null() {
            serde_json::json!({})
        } else {
            body.fields
        },
    };
    let seq = lb_host::telemetry_seed(&gw.node.store, &gw.node.bus, p.ws(), body.cap, &record)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({ "seq": seq })))
}

/// `POST /_seed/proof_panel` — install AND LOAD the REAL `proof-panel` wasm component into the token's
/// workspace, so its `proof-panel.proof.derive` tool is callable over the live `POST /mcp/call` bridge
/// (the host-callback slice). Unlike `/_seed/extension` (which only writes an Install RECORD for the UI
/// to list), this calls the real `install_extension` — persisting the grant AND loading the component
/// into the runtime, so the guest tool actually runs. The grant is the manifest's full requested set
/// (publisher = approver), so the guest's `caller ∩ grant` callbacks (`series.latest`/`ingest.write`)
/// are authorized. The real wasm bytes are read from the build output (built by build.sh).
async fn seed_proof_panel(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = auth(&gw, &headers).await?;
    const MANIFEST: &str = include_str!("../../../../extensions/proof-panel/extension.toml");
    let wasm_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/proof-panel/target/wasm32-wasip2/release/proof_panel_ext.wasm");
    let wasm = std::fs::read(&wasm_path).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "proof-panel wasm missing at {} ({e}) — build it: bash rust/extensions/proof-panel/build.sh",
                wasm_path.display()
            ),
        )
    })?;
    // The manifest's full requested cap set = the admin approval (publisher-as-approver). The guest's
    // callback authority is `caller ∩ this grant`; the dev session token holds the same caps.
    let approved = lb_ext_loader::Manifest::parse(MANIFEST)
        .map(|m| m.requested_caps)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    lb_host::install_extension(&gw.node, p.ws(), MANIFEST, &wasm, &approved, 0)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SeedSessionRequest {
    user: String,
    workspace: String,
    #[serde(default)]
    caps: Vec<String>,
}

#[derive(Serialize)]
struct SeedSessionReply {
    token: String,
    principal: String,
    workspace: String,
    caps: Vec<String>,
}

/// `POST /_seed/session` — mint a real signed token with an explicit cap set for frontend deny
/// tests. This does not fake a route or backend response; it only supplies a narrower authenticated
/// caller than the broad dev-login principal.
async fn seed_session(
    State(gw): State<Gateway>,
    Json(req): Json<SeedSessionRequest>,
) -> Result<Json<SeedSessionReply>, (StatusCode, String)> {
    if req.user.is_empty() || req.workspace.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "user and workspace required".into(),
        ));
    }
    let claims = Claims {
        sub: req.user.clone(),
        ws: req.workspace.clone(),
        role: Role::Member,
        caps: req.caps.clone(),
        iat: gw.now.saturating_sub(1),
        exp: gw.now.saturating_add(10_000),
    };
    Ok(Json(SeedSessionReply {
        token: mint(&gw.key, &claims),
        principal: req.user,
        workspace: req.workspace,
        caps: req.caps,
    }))
}

/// `POST /_seed/iot_demo` — seed the dashboard demo series (`cooler.temp`/`fryer.state`) + their tags
/// into the token's workspace through the **real ingest path** (dashboard scope, build step 1). Lets
/// the dashboard Vitest bind widgets to real, tagged series (incl. the `{find:{tags}}` binding).
async fn seed_iot_demo(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = auth(&gw, &headers).await?;
    let report = lb_host::seed_iot_demo(&gw.node.store, p.ws(), 1)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(serde_json::json!({
        "series": report.series,
        "committed": report.samples_committed,
    })))
}

async fn auth(
    gw: &Gateway,
    headers: &HeaderMap,
) -> Result<lb_auth::Principal, (StatusCode, String)> {
    authenticate(gw, headers)
        .await
        .map_err(|_| (StatusCode::UNAUTHORIZED, "bad token".into()))
}

/// `POST /_seed/inbox` — write a real durable inbox `Item` into the token's workspace.
async fn seed_inbox(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(item): Json<Item>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = auth(&gw, &headers).await?;
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
    let p = auth(&gw, &headers).await?;
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
    let p = auth(&gw, &headers).await?;
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
    let p = auth(&gw, &headers).await?;
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
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("ingest_write: {e:?}"),
            )
        })?;
    lb_host::drain_workspace(&gw.node.store, ws)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("drain: {e:?}")))?;

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

/// `POST /_seed/flow_node` body — install a real extension that contributes ONE `[[node]]` to the
/// token's workspace, so `flows.nodes` returns it (the palette hot-reload + ext-node tests). This is
/// **seeding, not faking** (testing-scope §3.1): it writes a real `Install` record carrying a real
/// `NodeBlock` + the granted cap the node's `tool` resolves to, exactly the path a real install
/// persists. The node's `tool` MUST be in the granted set or `flows.nodes` drops it (no install grant).
#[derive(Debug, Deserialize)]
struct SeedFlowNode {
    ext: String,
    /// The `mcp:<ext>.<tool>:call` cap to grant (the node's bound tool). Member-level grant.
    tool_cap: String,
    node: NodeBlock,
}

async fn seed_flow_node(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<SeedFlowNode>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = auth(&gw, &headers).await?;
    // A real install record: the granted cap lets the node's tool run (`caller ∩ install-grant`),
    // and the `nodes` block is the read-time union `flows.nodes` walks. `Manifest::parse` is NOT
    // needed — the block is validated defensively at `flows.nodes` time, exactly as production.
    let mut install = Install::new(body.ext.clone(), "0.1.0", vec![body.tool_cap], gw.now);
    install.nodes = vec![body.node];
    record_install(&gw.node.store, p.ws(), &install)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}
