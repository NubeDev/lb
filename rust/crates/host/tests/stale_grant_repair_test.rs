//! Stale-grant repair at the dispatch gate — the sibling of the builtin-role-freshness fix.
//!
//! A session token is a CACHED projection of `resolve_caps` taken at login. A grant written
//! afterwards is invisible to it until it expires (12h). One such write happens on a completely
//! routine action: `grant_ui_scope_to_admin` runs on every extension install — so an extension
//! upgrade that adds a verb left every already-logged-in admin denied, with the page silently
//! degrading and nothing in any log to explain it.
//!
//! `refresh_grants_if_denied` was written to close that and was never wired to a gate; this suite
//! covers it now that `dispatch_at_depth` calls it. Real infra (rule #9): a booted node, real
//! grants written to the real store, dispatched through the real `call_tool` seam.
//!
//! The four properties, in the order they matter:
//!   1. it REPAIRS — a token minted before the grant can call the verb;
//!   2. it does not WIDEN — a caller with no such grant is still denied, identically;
//!   3. it does not resurrect authority for a DELEGATED principal (an agent must not outgrow the
//!      delegation it was created with);
//!   4. the repaired identity reaches the verb's OWN inner cap re-check, not just the outer gate.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{grant_assign, Subject};
use lb_host::{call_tool, Node};
use serde_json::json;

/// Mint a token carrying exactly `caps` — a login-time snapshot, which is the thing that goes stale.
fn token_with(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    principal_inner(sub, ws, caps, None)
}

/// A DELEGATED principal (an agent acting for a human): `constraint` is set, which
/// `with_live_grants` refuses to widen.
fn delegated(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    principal_inner(
        sub,
        ws,
        caps,
        Some(caps.iter().map(|c| (*c).to_string()).collect()),
    )
}

fn principal_inner(
    sub: &str,
    ws: &str,
    caps: &[&str],
    constraint: Option<Vec<String>>,
) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// Write a durable grant AFTER the token above was minted — the extension-install shape. Uses the
/// real `grant_assign` write path, not a hand-built record, so the test cannot pass against a row
/// shape the resolver would not actually accept (rule #9).
async fn grant(node: &Arc<Node>, ws: &str, bare_user: &str, cap: &str) {
    grant_assign(&node.store, ws, &Subject::User(bare_user.to_string()), cap)
        .await
        .expect("grant is written");
}

/// 1. THE REPAIR. A token minted before the grant existed can call the verb — no re-login.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_grant_written_after_login_reaches_an_already_minted_token() {
    let ws = "stale-repair";
    let node = Arc::new(Node::boot().await.expect("node boots"));

    // The login-time snapshot: enough to exist, but NOT the verb under test.
    let p = token_with("user:test", ws, &["store:*:read", "store:*:write"]);
    let call = || json!({ "id": "d1", "title": "Ops", "cells": [], "now": 1 }).to_string();

    // Before the grant: denied, as it should be.
    let err = call_tool(&node, &p, ws, "dashboard.save", &call())
        .await
        .expect_err("no grant yet");
    assert!(err.to_string().contains("denied"), "opaque denial: {err}");

    // The extension-install shape: a durable grant written to the store, long after the token.
    grant(&node, ws, "test", "mcp:dashboard.save:call").await;

    // The SAME stale token now works — this is the whole point.
    call_tool(&node, &p, ws, "dashboard.save", &call())
        .await
        .expect("the stale token is repaired from the durable grant store");
}

/// 2. NO WIDENING. The repair must not turn "denied" into "allowed" for a caller who genuinely
/// holds nothing — and the refusal must be the identical opaque one, since a *different* error
/// would itself signal that a grant exists.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_caller_with_no_durable_grant_is_still_denied_identically() {
    let ws = "stale-nowiden";
    let node = Arc::new(Node::boot().await.expect("node boots"));

    // bob holds nothing; test is granted the verb. The store therefore HAS a grant row for this
    // cap — just not for bob — so a repair that keyed on the cap rather than the subject would
    // wrongly let bob through here.
    grant(&node, ws, "test", "mcp:dashboard.save:call").await;
    let bob = token_with("user:bob", ws, &["store:*:read"]);

    let err = call_tool(
        &node,
        &bob,
        ws,
        "dashboard.save",
        &json!({ "id": "d1", "title": "x", "cells": [], "now": 1 }).to_string(),
    )
    .await
    .expect_err("bob holds no such grant");
    assert!(
        err.to_string().contains("denied"),
        "a genuine denial stays the ordinary opaque denial: {err}"
    );
}

/// 3. NO RESURRECTION FOR A DELEGATED PRINCIPAL. An agent's caps were deliberately narrowed to
/// `agent ∩ caller`; re-widening from the human's grant store would let it outgrow the delegation
/// it was created with. `with_live_grants` refuses this, and the repair must not route around it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_delegated_principal_is_never_rewidened_from_the_grant_store() {
    let ws = "stale-delegated";
    let node = Arc::new(Node::boot().await.expect("node boots"));

    // The human behind the agent HAS the grant durably...
    grant(&node, ws, "test", "mcp:dashboard.save:call").await;
    // ...but the agent acting for her was delegated WITHOUT it.
    let agent = delegated("user:test", ws, &["store:*:read", "store:*:write"]);

    let err = call_tool(
        &node,
        &agent,
        ws,
        "dashboard.save",
        &json!({ "id": "d1", "title": "x", "cells": [], "now": 1 }).to_string(),
    )
    .await
    .expect_err("a delegated principal must not re-widen from its delegator's grants");
    assert!(err.to_string().contains("denied"), "opaque denial: {err}");
}

/// 4. THE REPAIRED IDENTITY REACHES THE VERB, not just the outer gate. Host verbs re-check their own
/// capability internally, so a repair that fixed only the dispatcher's gate would pass it and then
/// be denied *inside* — the call would still fail, and the repair would look like it worked. Proven
/// with a verb whose service layer re-runs its own `authorize_tool`: a successful call is only
/// possible if BOTH gates saw the refreshed principal.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_refreshed_principal_reaches_the_verbs_own_inner_gate() {
    let ws = "stale-inner";
    let node = Arc::new(Node::boot().await.expect("node boots"));

    let p = token_with("user:test", ws, &["store:*:read", "store:*:write"]);
    grant(&node, ws, "test", "mcp:dashboard.save:call").await;
    grant(&node, ws, "test", "mcp:dashboard.get:call").await;

    call_tool(
        &node,
        &p,
        ws,
        "dashboard.save",
        &json!({ "id": "inner", "title": "Ops", "cells": [], "now": 1 }).to_string(),
    )
    .await
    .expect("save passes the outer gate AND dashboard's own inner authorize");

    // `dashboard.get` re-checks its cap inside the service too, and reads back what was written —
    // so a green read here proves the repaired identity survived the whole path, twice over.
    let out = call_tool(
        &node,
        &p,
        ws,
        "dashboard.get",
        &json!({ "id": "inner" }).to_string(),
    )
    .await
    .expect("the read is repaired the same way");
    assert!(out.contains("Ops"), "the record round-tripped: {out}");
}
