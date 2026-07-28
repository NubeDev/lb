//! Version-history routes — the shell's "what did this look like before?" affordance over the
//! gateway (versions scope, #112). Mirrors the `undo.rs` / `flows.rs` typed-route precedent: each
//! route maps 1:1 onto a shipped host verb and re-checks its `mcp:<verb>:call` capability
//! server-side via `lb_host::call_tool` (the same MCP chokepoint every other caller uses).
//!
//! The workspace + principal come from the **token**, never the body or the path (§7) — so a session
//! can only ever reach its own workspace's rings, and there is no `ws` parameter here to get wrong.
//!
//! `kind` travels as an OPAQUE path segment. The gateway does not know which kinds exist and must
//! not: the host's kind plan table is the one place that decides, and an unknown kind comes back as
//! the host's typed `400` naming the kinds that do exist. That is what keeps a future kind a
//! zero-change addition here.

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_mcp::ToolError;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::session::authenticate;
use crate::state::Gateway;

/// `GET /versions/{kind}/{id}?limit=` — the entity's ring, newest-first, **metadata only**.
/// Gated `mcp:versions.list:call` (viewer tier — seeing history you cannot restore is correct).
#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn get_versions(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((kind, id)): Path<(String, String)>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let mut input = json!({ "kind": kind, "id": id });
    if let Some(limit) = q.limit {
        input["limit"] = json!(limit);
    }
    call(&gw, &p, "versions.list", &input).await
}

/// `GET /versions/{kind}/{id}/{version_id}` — one version's full snapshot, fetched lazily once a
/// caller has picked a row. Gated `mcp:versions.get:call`.
pub async fn get_version(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((kind, id, version_id)): Path<(String, String, String)>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(
        &gw,
        &p,
        "versions.get",
        &json!({ "kind": kind, "id": id, "version_id": version_id }),
    )
    .await
}

/// `POST /versions/{kind}/{id}/{version_id}/restore` — make that version live again by re-saving it.
/// Gated `mcp:versions.restore:call` PLUS the host's no-escalation check on the kind's own save cap,
/// so a caller who could not perform the save directly is refused here too.
///
/// A restore that the kind's save verb REFUSES (a snapshot made invalid by a since-tightened
/// validator) is a `400` carrying the validator's own words — the caller's fix is to pick a
/// different version, and a `500` would read as "the node is broken".
#[derive(Debug, Default, Deserialize)]
pub struct RestoreBody {
    /// Optional logical clock for the restore (unix seconds). Absent = the node stamps it.
    #[serde(default)]
    pub now: Option<u64>,
}

/// The body is taken as raw [`Bytes`], NOT as `Option<Json<RestoreBody>>`.
///
/// Every field of `RestoreBody` is optional, so "no body" is the ordinary call — and that is exactly
/// what a client writing `POST .../restore` with a `content-type: application/json` header and no
/// payload sends. `Option<Json<T>>` rejects that shape in the EXTRACTOR, before the handler runs, so
/// the caller got a `400` for a request that is completely valid (found by the live E2E walk). Worse,
/// an extractor-stage rejection happens before the capability gate inside `versions.restore`, which
/// inverts this codebase's deliberate ordering: a deny must be an opaque `403`, and answering "your
/// body is malformed" first tells an unauthorized caller something about the request shape.
///
/// So: an absent or empty body means "no options"; a non-empty body is parsed here, inside the
/// handler, and only a genuinely malformed one is a `400` — a caller who *did* send `now` is never
/// silently ignored.
fn parse_restore_body(body: &Bytes) -> Result<RestoreBody, (StatusCode, String)> {
    if body.iter().all(u8::is_ascii_whitespace) {
        return Ok(RestoreBody::default());
    }
    serde_json::from_slice(body).map_err(|e| (StatusCode::BAD_REQUEST, format!("body: {e}")))
}

pub async fn post_version_restore(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path((kind, id, version_id)): Path<(String, String, String)>,
    body: Bytes,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let mut input = json!({ "kind": kind, "id": id, "version_id": version_id });
    if let Some(now) = parse_restore_body(&body)?.now {
        input["now"] = json!(now);
    }
    call(&gw, &p, "versions.restore", &input).await
}

/// `GET /versions/config` — how many versions this workspace keeps, plus the node's own bounds so a
/// client renders them without hardcoding. Gated `mcp:versions.config.get:call`.
///
/// Route ordering note: this is a TWO-segment path (`/versions/config`) while the list route is
/// three (`/versions/{kind}/{id}`), so the two cannot shadow each other whatever order they are
/// registered in — deliberate, rather than relying on a matcher's precedence rules.
pub async fn get_versions_config(
    State(gw): State<Gateway>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "versions.config.get", &json!({})).await
}

/// `PUT /versions/config` — set the cap. Admin-only (`mcp:versions.config.set:call`). MERGES: an
/// omitted field is left as stored, so a client that only knows about `cap` cannot blank another
/// client's `per_kind` overrides.
pub async fn put_versions_config(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    call(&gw, &p, "versions.config.set", &body).await
}

/// Forward one versions MCP call through the host (re-checking the cap), returning its JSON output.
async fn call(
    gw: &Gateway,
    p: &lb_auth::Principal,
    tool: &str,
    input: &Value,
) -> Result<Json<Value>, (StatusCode, String)> {
    let out = lb_host::call_tool(&gw.node, p, p.ws(), tool, &input.to_string())
        .await
        .map_err(status)?;
    let value: Value = serde_json::from_str(&out).unwrap_or(Value::String(out));
    Ok(Json(value))
}

/// Map an MCP gate outcome onto HTTP. `Denied` → opaque `403` (no existence signal — a caller
/// refused `versions.get` learns nothing about whether the version exists); `BadInput` → `400`
/// verbatim (the unknown-kind message names the kinds that DO exist, and a refused restore carries
/// the save verb's own validator error); a store fault → `500`.
fn status(e: ToolError) -> (StatusCode, String) {
    match e {
        ToolError::Denied | ToolError::DeniedBecause { .. } => {
            (StatusCode::FORBIDDEN, "not permitted".into())
        }
        ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        ToolError::NotFound => (StatusCode::NOT_FOUND, "no such version".into()),
        ToolError::Extension(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        // These verbs are host-native and always local, so there is no node to address. Mapped to
        // 500 rather than swallowed: one appearing here is a real bug in verb routing.
        e @ (ToolError::Ambiguous { .. }
        | ToolError::NodeUnreachable { .. }
        | ToolError::NodeTooOld { .. }) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape the live walk found broken: `POST .../restore` with a JSON content-type and NO
    /// payload is the ordinary call (every option is optional), and must not be a `400`.
    #[test]
    fn an_absent_or_empty_body_means_no_options() {
        for raw in ["", "   ", "\n"] {
            let b =
                parse_restore_body(&Bytes::from(raw)).expect("an empty body is a valid restore");
            assert!(b.now.is_none());
        }
        assert!(parse_restore_body(&Bytes::from("{}"))
            .expect("{} parses")
            .now
            .is_none());
    }

    /// A caller who DID send a clock is honoured — the lenient empty case must not become a blanket
    /// "ignore whatever they sent".
    #[test]
    fn a_supplied_now_is_carried_through() {
        let b = parse_restore_body(&Bytes::from(r#"{"now":1700000000}"#)).expect("parses");
        assert_eq!(b.now, Some(1_700_000_000));
    }

    /// ...and a genuinely malformed body is still a `400`, not silently defaulted.
    #[test]
    fn a_malformed_body_is_still_rejected() {
        let (code, _) = parse_restore_body(&Bytes::from("{not json")).unwrap_err();
        assert_eq!(code, StatusCode::BAD_REQUEST);
    }
}
