//! Invite admin routes — the authenticated (browser) half of the shipped invite verb family
//! (invite-admin-routes scope). Mirror `lb_host::invite_*` 1:1: mint, list, revoke, resend. The
//! pre-auth redeem half (`POST /public/invite/accept`, `GET /public/invite/verify`) lives in
//! `invite_accept.rs`; these are its authenticated counterpart.
//!
//! Every route authenticates the session token and takes the workspace from the principal, never
//! from the body or the path — a forged cross-workspace mint/list/revoke is impossible to express.
//! Authorization stays where it already lives, inside the host verb: `invite.list` is gated
//! `mcp:invite.list:call`, `invite.create`/`revoke`/`resend` are gated `mcp:invite.create:call`. A
//! second gate here would be a place for the two to drift apart.
//!
//! The raw token is returned ONCE by `create`/`resend` and is never stored, never logged, and never
//! present in `list` (the record carries only its SHA-256 `token_hash`).

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{Invite, InviteStatus};
use serde::Deserialize;

use crate::session::authenticate;
use crate::state::Gateway;

/// The `POST /admin/invites` body — the argument set `invite_create` already takes. `role`/`team`
/// default to the empty string (= grant none); `expires_ts` defaults to `0` (= never expires).
#[derive(Debug, Deserialize)]
pub struct CreateInvite {
    pub email: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub team: String,
    #[serde(default)]
    pub payload: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub expires_ts: u64,
}

/// `POST /admin/invites` — mint an invite for the session's workspace. Returns `{ token }`: the
/// plaintext, **once** (only its hash is stored, so it is unrecoverable afterwards).
pub async fn create_invite(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<CreateInvite>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let token = lb_host::invite_create(
        &gw.node.store,
        &p,
        p.ws(),
        &body.email,
        &body.role,
        &body.team,
        body.payload.as_deref(),
        body.locale.as_deref(),
        body.expires_ts,
        gw.now(),
    )
    .await
    .map_err(invite_err)?;
    Ok(Json(serde_json::json!({ "token": token })))
}

/// The `GET /admin/invites` query — the optional status filter.
#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// One of `pending` | `accepted` | `revoked` | `expired`. Absent = the whole roster.
    #[serde(default)]
    pub status: Option<String>,
}

/// `GET /admin/invites` — the invite roster for the session's workspace, optionally filtered by
/// `?status=`. The records are `lb_authz::Invite`, which by construction carries only the
/// `token_hash` — never a redeemable token, for any status.
///
/// The filter runs AFTER the host call (the host verb is the authorization + workspace boundary;
/// a filter is presentation). It matches the **effective** status, not the stored one: expiry is
/// wall-clock, so a record still stored as `pending` past its `expires_ts` reads as `expired` here
/// rather than being offered to a console as actionable.
pub async fn list_invites(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<Invite>>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let want = q.status.as_deref().map(parse_status).transpose()?;
    let now = gw.now();
    let mut invites = lb_host::invite_list(&gw.node.store, &p, p.ws())
        .await
        .map_err(invite_err)?;
    if let Some(want) = want {
        invites.retain(|i| effective_status(i, now) == want);
    }
    Ok(Json(invites))
}

/// `POST /admin/invites/{token_hash}/revoke` — kill a pending invite. The host verb is idempotent
/// and reports whether anything matched; the route maps "nothing matched" to `404` so a stale or
/// unknown hash reads as not-found. `204` on a real revoke.
pub async fn revoke_invite(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(token_hash): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let revoked = lb_host::invite_revoke(&gw.node.store, &p, p.ws(), &token_hash)
        .await
        .map_err(invite_err)?;
    if revoked {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(not_found())
    }
}

/// `POST /admin/invites/{token_hash}/resend` — rotate the token. Returns a fresh `{ token }`; the
/// prior one is dead from this moment (the console must say so — see the scope's footgun note).
pub async fn resend_invite(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(token_hash): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let p = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    let token = lb_host::invite_resend(&gw.node.store, &p, p.ws(), &token_hash, gw.now())
        .await
        .map_err(invite_err)?;
    Ok(Json(serde_json::json!({ "token": token })))
}

/// The effective status of a record at `now`: the stored status, except that a `pending` record
/// past its expiry is `expired`. `Invite::is_redeemable` is the same predicate the accept chain
/// uses, so "pending here" and "redeemable there" cannot drift.
fn effective_status(invite: &Invite, now: u64) -> InviteStatus {
    if invite.status == InviteStatus::Pending && !invite.is_redeemable(now) {
        InviteStatus::Expired
    } else {
        invite.status.clone()
    }
}

/// Parse a `?status=` value. An unknown value is a caller error (`400`), never a silent empty list.
fn parse_status(s: &str) -> Result<InviteStatus, (StatusCode, String)> {
    match s {
        "pending" => Ok(InviteStatus::Pending),
        "accepted" => Ok(InviteStatus::Accepted),
        "revoked" => Ok(InviteStatus::Revoked),
        "expired" => Ok(InviteStatus::Expired),
        other => Err((
            StatusCode::BAD_REQUEST,
            format!("unknown status '{other}' (pending|accepted|revoked|expired)"),
        )),
    }
}

/// The ONE uniform not-found reply. Deliberately identical for missing / revoked / expired /
/// already-accepted — the same token-oracle reasoning as the pre-auth routes: an admin surface must
/// not become a way to probe which hashes ever existed in another workspace.
fn not_found() -> (StatusCode, String) {
    (StatusCode::NOT_FOUND, "invite not found".into())
}

/// Map an invite-service error to HTTP: a denial is `403`, bad input `400`, any
/// unknown/unusable-token outcome the uniform `404`, and everything else `500`.
fn invite_err(e: lb_host::InviteError) -> (StatusCode, String) {
    match e {
        lb_host::InviteError::Denied => (StatusCode::FORBIDDEN, "denied".into()),
        lb_host::InviteError::BadInput(m) => (StatusCode::BAD_REQUEST, m),
        lb_host::InviteError::NotFound
        | lb_host::InviteError::BadToken
        | lb_host::InviteError::Expired
        | lb_host::InviteError::AlreadyAccepted
        | lb_host::InviteError::Revoked => not_found(),
        // `Store` — and `IdentityExists`, which only the pre-auth accept chain can raise — are
        // server-side failures from these four routes' point of view.
        other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
    }
}
