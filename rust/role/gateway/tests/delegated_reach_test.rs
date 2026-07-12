//! Delegated reach — the `subject` half of native-caller-identity scope, GAP B (option (a)).
//! `authz.check_scoped` / `authz.scope_filter` gain an optional `subject`: present ⇒ resolve THAT
//! subject's scoped reach, but ONLY if the caller holds the delegation marker cap
//! `mcp:authz.delegate_reach:call`; absent ⇒ today's exact behaviour (the caller's own reach).
//!
//! Real node, real store, real axum gateway over `POST /mcp/call` — no mocks (CLAUDE §9 / testing §0).
//! A scoped grant is seeded through the REAL `POST /admin/grants` route so reach resolves against a
//! real record. Covers the scope's mandatory categories:
//!   - **delegated reach — allow:** a caller WITH the delegation cap gets the SUBJECT's reach, not its
//!     own (the sidecar-for-a-guardian flow).
//!   - **delegated reach — deny (the sacred one):** a caller WITHOUT it is a 403 on a present
//!     `subject` — never a silent fallback to the caller's own reach (fail closed).
//!   - **absent subject is unchanged:** no `subject`, no delegation cap needed — the caller's own reach.
//!   - **cross-workspace isolation:** a `subject` resolved from a ws-B caller sees none of ws-A's grants.

mod common;

use axum::http::StatusCode;
use common::{bearer, gateway, json_body, json_post, token};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt;

/// The reach cap the guardian-style subject holds, scoped to a single child row.
const REACH_CAP: &str = "mcp:care.child.get:call";
/// The delegation marker a caller must hold to name a `subject` other than itself.
const DELEGATE_CAP: &str = "mcp:authz.delegate_reach:call";

/// Seed a scoped grant: `subject` holds `REACH_CAP` on `child` row `id`, in `ws`, via the real
/// admin route. The admin must hold the grant cap AND the cap being granted (no-widening).
async fn seed_scoped_grant(
    gw: &lb_role_gateway::Gateway,
    key: &lb_auth::SigningKey,
    ws: &str,
    subject: &str,
    id: &str,
) {
    let admin = token(
        key,
        "user:admin",
        ws,
        &["mcp:grants.assign:call", REACH_CAP],
    );
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({
                    "subject": subject,
                    "cap": REACH_CAP,
                    "scope": { "kind": "ids", "table": "child", "ids": [id] },
                }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "seed grant persists");
}

/// `POST /mcp/call` as `bearer` with `{tool, args}`; return `(status, json)`.
async fn mcp_call(
    gw: &lb_role_gateway::Gateway,
    bearer_tok: &str,
    tool: &str,
    args: Value,
) -> (StatusCode, Value) {
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/mcp/call", json!({ "tool": tool, "args": args })),
            bearer_tok,
        ))
        .await
        .unwrap();
    let status = resp.status();
    let body = if status == StatusCode::OK {
        json_body::<Value>(resp).await
    } else {
        Value::Null
    };
    (status, body)
}

// ── Delegated reach — ALLOW ──────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delegated_check_scoped_resolves_the_subject_not_the_caller() {
    let (gw, key) = gateway().await;
    let ws = "acme";
    // ana (the guardian/subject) may reach child:leo; the caller (the extension identity) holds NO
    // reach grant of its own — only the reach-verb cap + the delegation cap.
    seed_scoped_grant(&gw, &key, ws, "user:ana", "leo").await;

    let ext = token(
        &key,
        "ext:care",
        ws,
        &["mcp:authz.check_scoped:call", DELEGATE_CAP],
    );

    // subject = ana → allowed for leo (ana's grant), even though the CALLER holds no reach at all.
    let (status, body) = mcp_call(
        &gw,
        &ext,
        "authz.check_scoped",
        json!({ "cap": REACH_CAP, "table": "child", "id": "leo", "subject": "user:ana" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "delegated check dispatches: {body}");
    assert_eq!(
        body["allowed"], true,
        "ana reaches leo via her scoped grant"
    );

    // subject = mallory (no grant) → not allowed, even to the same delegated caller.
    let (status, body) = mcp_call(
        &gw,
        &ext,
        "authz.check_scoped",
        json!({ "cap": REACH_CAP, "table": "child", "id": "leo", "subject": "user:mallory" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["allowed"], false, "mallory has no edge to leo");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delegated_scope_filter_returns_the_subjects_rows() {
    let (gw, key) = gateway().await;
    let ws = "acme";
    seed_scoped_grant(&gw, &key, ws, "user:ana", "leo").await;

    let ext = token(
        &key,
        "ext:care",
        ws,
        &["mcp:authz.scope_filter:call", DELEGATE_CAP],
    );
    let (status, body) = mcp_call(
        &gw,
        &ext,
        "authz.scope_filter",
        json!({ "cap": REACH_CAP, "table": "child", "subject": "user:ana" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "delegated filter dispatches: {body}"
    );
    assert_eq!(
        body["filter"]["ids"],
        json!(["leo"]),
        "the filter is ANA's row set, not the caller's"
    );
}

// ── Delegated reach — DENY (the sacred one) ──────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn subject_without_delegation_cap_is_denied_never_falls_back() {
    let (gw, key) = gateway().await;
    let ws = "acme";
    seed_scoped_grant(&gw, &key, ws, "user:ana", "leo").await;

    // The caller holds the reach VERB cap but NOT the delegation cap. Naming a `subject` must be a
    // hard 403 — never a silent fallback to the caller's OWN reach (which would leak or mis-attribute).
    let ext = token(&key, "ext:care", ws, &["mcp:authz.check_scoped:call"]);
    let (status, _body) = mcp_call(
        &gw,
        &ext,
        "authz.check_scoped",
        json!({ "cap": REACH_CAP, "table": "child", "id": "leo", "subject": "user:ana" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a subject without the delegation cap fails CLOSED (the sacred deny)"
    );

    // scope_filter half — same deny, no silent empty/own-reach fallback.
    let ext_f = token(&key, "ext:care", ws, &["mcp:authz.scope_filter:call"]);
    let (status, _body) = mcp_call(
        &gw,
        &ext_f,
        "authz.scope_filter",
        json!({ "cap": REACH_CAP, "table": "child", "subject": "user:ana" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "scope_filter subject also fails closed"
    );
}

// ── Absent subject → today's exact behaviour (no delegation cap needed) ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn absent_subject_resolves_the_callers_own_reach_without_the_delegation_cap() {
    let (gw, key) = gateway().await;
    let ws = "acme";
    // The CALLER itself (user:sam) holds a scoped grant; it names no subject and holds no delegation
    // cap — the original behaviour, byte-for-byte, every existing call site unchanged.
    seed_scoped_grant(&gw, &key, ws, "user:sam", "mia").await;

    let sam = token(&key, "user:sam", ws, &["mcp:authz.check_scoped:call"]);
    let (status, body) = mcp_call(
        &gw,
        &sam,
        "authz.check_scoped",
        json!({ "cap": REACH_CAP, "table": "child", "id": "mia" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "own-reach check dispatches: {body}");
    assert_eq!(body["allowed"], true, "sam reaches his own scoped row");
}

// ── Cross-workspace isolation ────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn delegated_subject_never_crosses_the_workspace_wall() {
    let (gw, key) = gateway().await;
    // ana's grant lives in ws-A.
    seed_scoped_grant(&gw, &key, "ws-a", "user:ana", "leo").await;

    // A ws-B extension holding the delegation cap asks about ana — the subject resolves only within
    // the CALLER's workspace (ws-B), where ana has no grant, so leo is unreachable. The ws-A grant is
    // physically unreachable (resolution reads only the caller's namespace; the id is ws-relative).
    let ext_b = token(
        &key,
        "ext:care",
        "ws-b",
        &["mcp:authz.check_scoped:call", DELEGATE_CAP],
    );
    let (status, body) = mcp_call(
        &gw,
        &ext_b,
        "authz.check_scoped",
        json!({ "cap": REACH_CAP, "table": "child", "id": "leo", "subject": "user:ana" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["allowed"], false,
        "ws-B sees none of ws-A's scoped grants — the wall holds"
    );
}
