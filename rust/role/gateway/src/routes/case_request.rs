//! The **contractor's three routes** — the public, session-less half of the case plane
//! (case-plane scope §"Gateway — the token principal").
//!
//! `GET /r/{token}` is **not here.** It is a page URL: the browser must be served the SPA at it,
//! and the SPA fallback already does that. The same path cannot return HTML to a browser and JSON
//! to that page's `fetch`, so the data lives on three routes of its own:
//!
//! | route | does |
//! |---|---|
//! | `GET  /public/case/request?token=…`           | the view payload |
//! | `POST /public/case/request/reply`             | the answer (token in the body) |
//! | `POST /public/case/request/attachment?token=…`| one file, multipart, → `{ id, name, size }` |
//!
//! **All three mint the same principal and then get out of the way.** `case_request_authenticate`
//! hashes the presented token, finds the request, and returns a principal holding exactly
//! `mcp:case.request.view:call` and `mcp:case.request.reply:call`, constrained to that one request
//! id. The first two routes then go through the ordinary `call_tool` chokepoint — the same gate, the
//! same workspace wall, the same audit as any other caller. Nothing here is a bypass; it is the
//! narrowest principal in the system walking through the front door.
//!
//! **The attachment route is a route, not a third verb.** It mints no new capability: it re-checks
//! the token's scope and writes the asset under the host's own authority
//! (`lb_host::case_request_attach`). Adding `mcp:case.request.attach:call` would widen the smallest
//! principal in the system so a contractor could send a photo.
//!
//! **410, and never which.** Withdrawn, expired, or replied-past-the-window all answer the same
//! `410 Gone` with a body that says the ask "may have been completed, withdrawn, or simply timed
//! out". Committing to one of those tells a prober that the string they presented was a real token.
//! An unknown token is a plain `404`.
//!
//! **Rate-limited per client IP** by the same fixed-window limiter `/public/invite/accept` uses
//! (its own bucket — see `rate_limit.rs`), because a route that hashes a presented secret and
//! answers differently per outcome is an oracle.
//!
//! Every view is logged: who (`party:{id}`), which request, which workspace. A tokenised page is
//! the one surface with no login behind it, so the access log IS the audit trail.

use axum::extract::{Multipart, Query, State};
use axum::http::StatusCode;
use axum::Json;
use lb_host::{call_tool, RequestTokenError};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::state::Gateway;

/// The body a `410` answers with. One sentence, three possibilities, no commitment to which.
const GONE_BODY: &str = "This request link is no longer active — it may have been completed, \
                         withdrawn, or simply timed out.";

/// What a token that resolves to nothing answers with. Deliberately not a login prompt: there is no
/// account behind a request link and offering one would be an invitation to guess.
const NOT_FOUND_BODY: &str = "This request link is not valid.";

/// The largest attachment the public route accepts, before multipart framing. The host re-checks
/// against its own `MAX_ASSET_BYTES`; this is the transport-level bound so a body that could never
/// be stored is refused before it is buffered.
pub const MAX_ATTACHMENT_BYTES: usize = 8 * 1024 * 1024;

/// `?token=…` — the query both GET-shaped routes carry.
#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: String,
}

/// The reply body. The token rides in the body rather than the query so it does not land in an
/// access log or a `Referer` header on the way to recording an answer.
#[derive(Debug, Deserialize)]
pub struct ReplyBody {
    pub token: String,
    /// The reply itself (`kind`, and whatever that kind carries). Decoded by the verb, not here —
    /// the route must not become a second definition of what a reply is.
    pub reply: Value,
}

/// `GET /public/case/request?token=…` — what the tokenised page renders.
pub async fn get_case_request(
    State(gw): State<Gateway>,
    Query(q): Query<TokenQuery>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let (ws, principal, request_id) = resolve(&gw, &q.token).await?;
    tracing::info!(
        ws = %ws, party = %principal.sub(), request = %request_id,
        "case request: token principal viewed its request"
    );
    let out = call_tool(
        &gw.node,
        &principal,
        &ws,
        "case.request.view",
        &json!({ "id": request_id, "ts": gw.now() * 1000 }).to_string(),
    )
    .await
    .map_err(tool_err)?;
    Ok(Json(serde_json::from_str(&out).unwrap_or(Value::Null)))
}

/// `POST /public/case/request/reply` — the party's answer.
pub async fn post_case_request_reply(
    State(gw): State<Gateway>,
    Json(body): Json<ReplyBody>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let (ws, principal, request_id) = resolve(&gw, &body.token).await?;
    tracing::info!(
        ws = %ws, party = %principal.sub(), request = %request_id,
        "case request: token principal replied"
    );
    let out = call_tool(
        &gw.node,
        &principal,
        &ws,
        "case.request.reply",
        &json!({ "id": request_id, "reply": body.reply, "ts": gw.now() * 1000 }).to_string(),
    )
    .await
    .map_err(tool_err)?;
    Ok(Json(serde_json::from_str(&out).unwrap_or(Value::Null)))
}

/// `POST /public/case/request/attachment?token=…` — one file, `multipart/form-data`.
///
/// Returns `{ id, name, size }`; the `id` is what the reply's `attachments[]` must reference. It is
/// a ULID minted by the host, never the filename: a `.` in an asset id breaks
/// `store:asset/{id}:write` and the file becomes unreadable after a successful-looking upload.
pub async fn post_case_request_attachment(
    State(gw): State<Gateway>,
    Query(q): Query<TokenQuery>,
    multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, String)> {
    let (ws, principal, request_id) = resolve(&gw, &q.token).await?;
    let (name, mime, bytes) = read_one_file(multipart).await?;
    tracing::info!(
        ws = %ws, party = %principal.sub(), request = %request_id, bytes = bytes.len(),
        "case request: token principal uploaded an attachment"
    );
    let receipt = lb_host::case_request_attach(
        &gw.node.store,
        &principal,
        &ws,
        &request_id,
        &name,
        &mime,
        bytes,
        gw.now() * 1000,
    )
    .await
    .map_err(|e| match e {
        lb_host::CaseSvcError::Denied => (StatusCode::FORBIDDEN, "denied".to_string()),
        lb_host::CaseSvcError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        lb_host::CaseSvcError::Store(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
    })?;
    Ok(Json(serde_json::to_value(receipt).unwrap_or(Value::Null)))
}

/// Hash the presented token, resolve its request, and mint the token principal — the one step all
/// three routes share.
///
/// The workspace comes from the token itself (`lbr_{ws}.{secret}`): a public route has no session
/// and no path segment to learn the tenancy from, and the hash covers the whole string, so a
/// tampered workspace half simply resolves to nothing.
async fn resolve(
    gw: &Gateway,
    token: &str,
) -> Result<(String, lb_auth::Principal, String), (StatusCode, String)> {
    let Some(ws) = lb_host::workspace_of_token(token) else {
        return Err((StatusCode::NOT_FOUND, NOT_FOUND_BODY.to_string()));
    };
    let ws = ws.to_string();
    // The case plane's clock is epoch-MILLIS; the gateway's is seconds.
    let now_ms = gw.now() * 1000;
    match lb_host::case_request_authenticate(&gw.node.store, &ws, token, now_ms).await {
        Ok((principal, request)) => Ok((ws, principal, request.id)),
        Err(RequestTokenError::NotFound) => {
            Err((StatusCode::NOT_FOUND, NOT_FOUND_BODY.to_string()))
        }
        Err(RequestTokenError::Gone) => Err((StatusCode::GONE, GONE_BODY.to_string())),
        Err(RequestTokenError::Store(e)) => {
            tracing::warn!(ws = %ws, error = %e, "case request: token lookup failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "could not read the request".to_string(),
            ))
        }
    }
}

/// Pull the single file part out of a multipart body: the first part carrying a filename.
async fn read_one_file(
    mut multipart: Multipart,
) -> Result<(String, String, Vec<u8>), (StatusCode, String)> {
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("malformed multipart body: {e}"),
        )
    })? {
        let Some(name) = field.file_name().map(str::to_string) else {
            continue;
        };
        let mime = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = field
            .bytes()
            .await
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("could not read the file: {e}"),
                )
            })?
            .to_vec();
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err((
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("the file is larger than the {MAX_ATTACHMENT_BYTES}-byte limit"),
            ));
        }
        return Ok((name, mime, bytes));
    }
    Err((
        StatusCode::BAD_REQUEST,
        "the upload carried no file part".to_string(),
    ))
}

/// Map a tool error onto a status. A denial is opaque — the page never learns whether it was the
/// capability, the workspace or the request scope that refused.
fn tool_err(e: lb_mcp::ToolError) -> (StatusCode, String) {
    match e {
        lb_mcp::ToolError::Denied => (StatusCode::FORBIDDEN, "denied".to_string()),
        lb_mcp::ToolError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        lb_mcp::ToolError::NotFound => (StatusCode::NOT_FOUND, NOT_FOUND_BODY.to_string()),
        other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
    }
}
