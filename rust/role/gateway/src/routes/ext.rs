//! Extension lifecycle routes — the browser's `ext.*` surface over the gateway (lifecycle-management
//! scope: THE biggest real gap — the host had the verbs but only the Tauri shell reached them, so a
//! browser threw `unknown command`). Mirror `lb_host::ext_*` 1:1; gated server-side on
//! `mcp:ext.list:call` / `mcp:ext.disable:call` / `mcp:ext.uninstall:call`.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{ExtError, ExtRow};
use lb_registry::{Artifact, PublisherKey, TrustedKeys, Visibility};
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /extensions` — every installed extension (both tiers) with live state.
pub async fn list_extensions(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Vec<ExtRow>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let rows = lb_host::ext_list(&gw.node, &p, p.ws())
        .await
        .map_err(forbid)?;
    Ok(Json(rows))
}

/// `POST /extensions/{ext}/enable` — durable enable (eligible to auto-start on boot).
pub async fn enable_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ext): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::ext_enable(&gw.node, &p, p.ws(), &ext, gw.now())
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /extensions/{ext}/start` — **start a stopped extension now**, without bouncing the node.
///
/// The recovery `enable` only implied: `enable` flips durable intent but spawns nothing, and
/// `reset`/`restart` both need an existing sidecar handle — so before this route the only way to
/// start a stopped extension was to re-upload its artifact. Gated server-side on `mcp:ext.start:call`
/// inside `ext_start`. `200` with the outcome row (`spawned` + a `reason`, the same vocabulary the
/// boot log uses) — a refusal to start a *disabled* extension is a row, not an error: the durable
/// intent wins, and the caller is told which one it hit.
pub async fn start_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ext): Path<String>,
) -> Result<Json<lb_host::SpawnedExt>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let row = lb_host::ext_start(&gw.node, &lb_host::OsLauncher, &p, p.ws(), &ext, gw.now())
        .await
        .map_err(forbid)?;
    Ok(Json(row))
}

/// `POST /extensions/{ext}/disable` — durable disable (stop now + do-not-auto-start).
pub async fn disable_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ext): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::ext_disable(&gw.node, &p, p.ws(), &ext, gw.now())
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /extensions/{ext}/reset` — **re-arm** a native sidecar's exhausted restart budget and force
/// a fresh child (native-tier resilience). The rescue for a sidecar that crash-looped past
/// `max_restarts` and would otherwise return "restart budget exhausted" until the node is bounced.
/// Gated server-side on `mcp:native.reset:call` inside `reset_native`; `403` on a deny or if the
/// sidecar is not running here (wasm rows have no process — the host returns `NotRunning`).
pub async fn reset_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ext): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::reset_native(&gw.node, &lb_host::OsLauncher, &p, p.ws(), &ext, gw.now())
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /extensions/{ext}` — uninstall (stop/unload + delete the install record).
pub async fn uninstall_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(ext): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    lb_host::ext_uninstall(&gw.node, &p, p.ws(), &ext, gw.now())
        .await
        .map_err(forbid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /extensions` — **publish** (upload) a signed extension artifact (lifecycle-management scope:
/// the admin console's "publish an extension" path). Body is the [`Artifact`] verbatim (the same wire
/// shape the registry-host `POST /artifacts` accepts), including the publisher signature. The workspace
/// comes from the token, never the body (the hard wall, §7). Gated server-side on `mcp:ext.publish:call`
/// inside the host verb; verify-before-store inside it too. `204` on publish, `403` on a capability
/// deny, `422` on a verification failure (tampered/unsigned/foreign-key — nothing stored).
pub async fn publish_extension(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    // Descriptive over-limit reject (extension-upload-limit fix): the route-scoped `DefaultBodyLimit`
    // carries a small margin above the configured ceiling so a just-oversized artifact reaches here
    // (rather than the layer's bare "length limit exceeded"). Report the size AND the limit so an
    // operator sees exactly what to raise. The declared `Content-Length` is what curl/the browser send;
    // absent (chunked) uploads fall through to the layer's hard cap.
    if let Some(len) = content_length(&headers) {
        let limit = gw.max_extension_upload_bytes;
        if len > limit {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!(
                    "extension artifact {} exceeds the upload limit {} \
                     (raise LB_MAX_EXTENSION_UPLOAD_BYTES / BootConfig::max_extension_upload_bytes)",
                    human_bytes(len),
                    human_bytes(limit),
                ),
            ));
        }
    }
    let publish = publish_body(body, &gw.trusted)?;
    lb_host::ext_publish(
        &gw.node,
        &p,
        p.ws(),
        publish.artifact,
        &publish.trusted,
        Visibility::Private,
        gw.now(),
    )
    .await
    .map_err(publish_status)?;
    Ok(StatusCode::NO_CONTENT)
}

struct PublishInput {
    artifact: Artifact,
    trusted: TrustedKeys,
}

#[derive(Deserialize)]
struct DevkitPublish {
    path: String,
}

fn publish_body(
    body: serde_json::Value,
    trusted: &TrustedKeys,
) -> Result<PublishInput, (StatusCode, String)> {
    if let Ok(artifact) = serde_json::from_value::<Artifact>(body.clone()) {
        return Ok(PublishInput {
            artifact,
            trusted: trusted.clone(),
        });
    }
    let req: DevkitPublish = serde_json::from_value(body).map_err(|e| {
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("bad publish body: {e}"),
        )
    })?;
    let key_id = "dev-publisher";
    let key_path = lb_dir().join("keys").join("dev-publisher.key");
    let loaded = lb_devkit::load_or_create_key(&key_path).map_err(pack_status)?;
    let path = lb_devkit::resolve_under_root(&lb_devkit::default_devkit_root(), &req.path)
        .map_err(pack_status)?;
    let manifest_path = path.join("extension.toml");
    let manifest =
        std::fs::read_to_string(&manifest_path).map_err(|e| pack_io("read manifest", e))?;
    let inspect = lb_devkit::inspect_extension(&path).map_err(pack_status)?;
    let bytes_path = built_binary_path(&path, &inspect);
    let bytes = std::fs::read(&bytes_path).map_err(|e| pack_io("read build output", e))?;
    let artifact = lb_devkit::sign_artifact(manifest, bytes, key_id, &loaded.signing_key)
        .map_err(pack_status)?;
    let mut local_trusted = trusted.clone();
    let publisher = PublisherKey::from_bytes(&loaded.signing_key.verifying_key().to_bytes())
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;
    // The shortcut trusts only the node-owned LB_DIR publisher key it just used. The page never
    // supplies key material; a normal signed-artifact upload still verifies against gw.trusted.
    local_trusted.insert(key_id.to_string(), publisher);
    Ok(PublishInput {
        artifact,
        trusted: local_trusted,
    })
}

fn lb_dir() -> std::path::PathBuf {
    std::env::var("LB_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(".lazybones"))
}

fn built_binary_path(
    path: &std::path::Path,
    inspect: &lb_devkit::InspectReport,
) -> std::path::PathBuf {
    match inspect.tier {
        lb_devkit::Tier::Wasm => path
            .join("target/wasm32-wasip2/release")
            .join(format!("{}_ext.wasm", inspect.id.replace('-', "_"))),
        lb_devkit::Tier::Native => path.join("target/release").join(&inspect.id),
    }
}

fn pack_status(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::UNPROCESSABLE_ENTITY, e.to_string())
}

fn pack_io(action: &str, e: std::io::Error) -> (StatusCode, String) {
    (StatusCode::UNPROCESSABLE_ENTITY, format!("{action}: {e}"))
}

/// The declared request body size from the `Content-Length` header, if present and parseable. Used by
/// the publish route to reject an oversized artifact with a descriptive 413 before buffering it.
fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(axum::http::header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// Render a byte count as a human-friendly size (e.g. `480.0 MiB`) for the over-limit error message —
/// an operator reads "480 MiB exceeds 384 MiB", not raw byte counts.
fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

fn forbid(e: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::FORBIDDEN, e.to_string())
}

/// Map a publish error to a status: a capability/workspace deny is `403`; a verification failure is
/// `422` (the upload was well-formed but its signature/digest did not check out — distinct from "you
/// may not"); any store fault is `403`-opaque like the other ext routes.
fn publish_status(e: ExtError) -> (StatusCode, String) {
    match e {
        ExtError::Unverified => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()),
        other => forbid(other),
    }
}
