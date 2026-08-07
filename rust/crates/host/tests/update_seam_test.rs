//! The `update.*` family end to end (node-update scope §Seam 1) — a REAL node, the REAL MCP bridge
//! (`lb_host::call_tool`, so the real caps gate runs), the REAL store, and a REAL in-test
//! [`UpdateProvider`]. No mocks: a test-local implementation of a public trait is the seam working
//! as designed, not a fake backend (rule 9) — the same posture `EmailProvider`'s tests take.
//!
//! What is asserted, per the scope's testing plan:
//! - **capability deny** at the VERB, per verb: `update.read` alone cannot apply, and `update.apply`
//!   alone cannot `credential.set`;
//! - **unconfigured**: no provider ⇒ `{"supported": false}` and clean `Unsupported`s, never `404`;
//! - **first-use auto-enrolment** mints exactly once, seals, and never re-provisions;
//! - the credential appears in **no response body**, and a **sealed credential beats the env NAME**;
//! - the sealed value is unreadable via `secret.get` AND via `store.query` from every workspace.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_tool, Accepted, AvailableVersion, CredentialStatus, Node, UpdateConfig, UpdateCx,
    UpdateError, UpdateEvent, UpdateProvider, UpdateStatus, UPDATE_UNSUPPORTED_PREFIX,
    UPDATE_VERBS,
};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const WS: &str = "nube";
const SECRET_PATH: &str = "update/credential";
/// The value the in-test backend hands lb during enrolment. Every response body is asserted NOT to
/// contain it — that assertion is the whole point of the custody design.
const MINTED: &str = "rbd_MINTED_SECRET_VALUE_0001";

const READ: &str = "mcp:update.read:call";
const APPLY: &str = "mcp:update.apply:call";
const CREDENTIAL: &str = "mcp:update.credential:call";

// ---------------------------------------------------------------------------------------------
// The real in-test provider
// ---------------------------------------------------------------------------------------------

/// A real `UpdateProvider` over in-process state. It behaves like a backend: it counts its enrolment
/// handshakes (so "mints exactly once" is observable), records the credential lb handed it (so
/// "sealed beats env" is observable), and refuses a credential it did not issue.
#[derive(Default)]
struct TestProvider {
    provisions: AtomicUsize,
    /// The `cx.credential` of the last call — how the test sees WHICH credential lb resolved.
    last_credential: Mutex<Option<String>>,
    /// When set, `provision_credential` answers this instead of minting.
    provision_error: Option<UpdateError>,
}

impl TestProvider {
    fn record(&self, cx: &UpdateCx) {
        *self.last_credential.lock().unwrap() = cx.credential.clone();
    }
}

#[async_trait]
impl UpdateProvider for TestProvider {
    async fn status(&self, cx: &UpdateCx) -> Result<UpdateStatus, UpdateError> {
        self.record(cx);
        let mut s = UpdateStatus::unsupported();
        s.supported = true;
        s.backend = "test-supervisor".into();
        s.package = Some("lb-node".into());
        s.current_version = Some("0.1.0".into());
        s.signing_key_durable = false;
        s.quarantined = vec!["0.1.1".into()];
        s.target_matches_self = true;
        Ok(s)
    }

    async fn check(&self, cx: &UpdateCx) -> Result<Vec<AvailableVersion>, UpdateError> {
        self.record(cx);
        Ok(vec![AvailableVersion {
            version: "0.1.2".into(),
            size: Some(151_270_900),
            source: "remote".into(),
        }])
    }

    async fn apply(&self, cx: &UpdateCx, version: &str) -> Result<Accepted, UpdateError> {
        self.record(cx);
        if version == "0.1.1" {
            return Err(UpdateError::Conflict {
                reason: "quarantined".into(),
            });
        }
        if version != "0.1.2" {
            return Err(UpdateError::NotFound {
                version: version.into(),
            });
        }
        Ok(Accepted {
            tx: "tx-apply".into(),
        })
    }

    async fn rollback(&self, cx: &UpdateCx) -> Result<Accepted, UpdateError> {
        self.record(cx);
        Ok(Accepted {
            tx: "tx-rollback".into(),
        })
    }

    async fn history(&self, cx: &UpdateCx, limit: u32) -> Result<Vec<UpdateEvent>, UpdateError> {
        self.record(cx);
        Ok(vec![UpdateEvent {
            tx: "tx-apply".into(),
            at: "2026-08-04T00:00:00Z".into(),
            from: Some("0.1.0".into()),
            to: Some("0.1.2".into()),
            outcome: format!("committed:limit={limit}"),
            reason: None,
        }])
    }

    async fn provision_credential(&self, _code: Option<&str>) -> Result<String, UpdateError> {
        if let Some(e) = &self.provision_error {
            return Err(e.clone());
        }
        self.provisions.fetch_add(1, Ordering::SeqCst);
        Ok(MINTED.to_string())
    }

    async fn verify_credential(&self, candidate: &str) -> Result<(), UpdateError> {
        if candidate == MINTED {
            Ok(())
        } else {
            Err(UpdateError::Unauthorized {
                code_required: false,
            })
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------------------------

fn principal(sub: &str, caps: &[&str]) -> Principal {
    principal_in(sub, WS, caps)
}

fn principal_in(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::WorkspaceAdmin,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A real booted node with the update seam installed over `provider`, sealing into `WS`.
async fn node_with(provider: Arc<TestProvider>) -> (Arc<Node>, Arc<TestProvider>) {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let mut cfg = UpdateConfig::new(provider.clone());
    cfg.credential_secret = Some(SECRET_PATH.into());
    node.install_update(Some(cfg), WS);
    (node, provider)
}

/// Call a verb through the REAL bridge and parse its JSON reply.
async fn call(
    node: &Arc<Node>,
    p: &Principal,
    tool: &str,
    args: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, p.ws(), tool, &args.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap_or(Value::Null))
}

/// Every reply crossing the wire is asserted free of the credential — the custody invariant, checked
/// on the actual bytes rather than trusted.
fn assert_no_credential(v: &Value) {
    let s = v.to_string();
    assert!(
        !s.contains(MINTED),
        "the credential leaked into a response body: {s}"
    );
}

// ---------------------------------------------------------------------------------------------
// Capability deny — per verb, at the verb
// ---------------------------------------------------------------------------------------------

/// The read grant reaches the four read verbs and NOTHING else. Asserted at the verb through the
/// real bridge, never at a UI: the deny that matters is the one a `curl` hits.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_grant_reaches_only_the_read_verbs() {
    let (node, _) = node_with(Arc::new(TestProvider::default())).await;
    let reader = principal("user:reader", &[READ]);

    for verb in [
        "update.status",
        "update.check",
        "update.history",
        "update.credential.status",
    ] {
        let out = call(&node, &reader, verb, json!({})).await;
        assert!(out.is_ok(), "{verb} must be allowed by the read grant");
        assert_no_credential(&out.unwrap());
    }

    for (verb, args) in [
        ("update.apply", json!({"version": "0.1.2"})),
        ("update.rollback", json!({})),
        ("update.credential.set", json!({"value": MINTED})),
        ("update.credential.claim", json!({})),
    ] {
        assert!(
            matches!(
                call(&node, &reader, verb, args).await,
                Err(ToolError::Denied)
            ),
            "{verb} must be DENIED to a read-only principal"
        );
    }
}

/// `update.apply` alone cannot `credential.set` — the three grants split by blast radius, and
/// holding the backend's credential is its own radius.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_grant_cannot_enrol_and_credential_grant_cannot_apply() {
    let (node, _) = node_with(Arc::new(TestProvider::default())).await;

    let applier = principal("user:applier", &[APPLY]);
    assert!(matches!(
        call(
            &node,
            &applier,
            "update.credential.set",
            json!({"value": MINTED})
        )
        .await,
        Err(ToolError::Denied)
    ));
    assert!(matches!(
        call(&node, &applier, "update.credential.claim", json!({})).await,
        Err(ToolError::Denied)
    ));

    let enroller = principal("user:enroller", &[CREDENTIAL]);
    assert!(matches!(
        call(
            &node,
            &enroller,
            "update.apply",
            json!({"version": "0.1.2"})
        )
        .await,
        Err(ToolError::Denied)
    ));
    assert!(matches!(
        call(&node, &enroller, "update.rollback", json!({})).await,
        Err(ToolError::Denied)
    ));
}

/// A principal holding NONE of the three is denied every verb — including `update.status`, which is
/// honest-but-still-gated. The deny is the opaque `ToolError::Denied`, indistinguishable from a
/// missing tool.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_capless_principal_is_denied_every_verb_opaquely() {
    let (node, _) = node_with(Arc::new(TestProvider::default())).await;
    let nobody = principal("user:nobody", &["mcp:dashboard.list:call"]);
    for verb in UPDATE_VERBS {
        assert!(
            matches!(
                call(&node, &nobody, verb, json!({})).await,
                Err(ToolError::Denied)
            ),
            "{verb} must be denied opaquely to a principal with none of the three grants"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Unconfigured — no provider
// ---------------------------------------------------------------------------------------------

/// `update = None` ⇒ `update.status` answers `{"supported": false}` and every OTHER verb is a clean
/// `Unsupported` — **never** a `404`/`NotFound`, which would say "this build has no such verb"
/// instead of "this node cannot replace itself".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_provider_answers_unsupported_never_not_found() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin = principal("user:admin", &[READ, APPLY, CREDENTIAL]);

    let st = call(&node, &admin, "update.status", json!({}))
        .await
        .expect("status answers on an unconfigured node");
    assert_eq!(st["supported"], json!(false));
    assert_eq!(st["credential"]["configured"], json!(false));
    assert_eq!(st["credential"]["source"], json!("none"));

    // credential.status is likewise answerable: "is this node enrolled?" must be askable before the
    // answer is known.
    let cs = call(&node, &admin, "update.credential.status", json!({}))
        .await
        .expect("credential.status answers on an unconfigured node");
    assert_eq!(cs["configured"], json!(false));

    for (verb, args) in [
        ("update.check", json!({})),
        ("update.history", json!({})),
        ("update.apply", json!({"version": "0.1.2"})),
        ("update.rollback", json!({})),
        ("update.credential.set", json!({"value": "x"})),
        ("update.credential.claim", json!({})),
    ] {
        match call(&node, &admin, verb, args).await {
            Err(ToolError::Extension(m)) => assert!(
                m.starts_with(UPDATE_UNSUPPORTED_PREFIX),
                "{verb} must answer Unsupported, got: {m}"
            ),
            other => panic!("{verb} must answer Unsupported, got {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// First-use auto-enrolment
// ---------------------------------------------------------------------------------------------

/// The first verb that resolves the credential and finds nothing sealed mints ONE, seals it
/// host-owned, and never provisions again — the zero-touch path (scope decision 10). Asserted on the
/// provider's own handshake counter, which is the only place a second mint could hide.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_use_auto_enrolment_mints_exactly_once_and_never_resends() {
    let (node, prov) = node_with(Arc::new(TestProvider::default())).await;
    let admin = principal("user:admin", &[READ, APPLY]);

    let st = call(&node, &admin, "update.status", json!({}))
        .await
        .expect("status");
    assert_eq!(prov.provisions.load(Ordering::SeqCst), 1, "minted once");
    assert_eq!(st["credential"]["configured"], json!(true));
    assert_eq!(st["credential"]["source"], json!("sealed"));
    assert!(st["credential"]["fingerprint"].is_string());
    assert_no_credential(&st);

    // Four more verbs, each of which resolves the credential: still exactly one handshake, and the
    // provider keeps receiving the SAME sealed value.
    for verb in ["update.status", "update.check", "update.history"] {
        assert_no_credential(&call(&node, &admin, verb, json!({})).await.expect(verb));
    }
    call(&node, &admin, "update.apply", json!({"version": "0.1.2"}))
        .await
        .expect("apply");
    assert_eq!(
        prov.provisions.load(Ordering::SeqCst),
        1,
        "auto-enrolment must never re-provision once a credential is sealed"
    );
    assert_eq!(
        prov.last_credential.lock().unwrap().as_deref(),
        Some(MINTED),
        "the sealed credential is what the provider receives on every later call"
    );
}

/// Concurrent triggers serialize on the seal: the loser re-resolves and finds the winner's secret, so
/// exactly ONE credential is ever minted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_triggers_seal_exactly_one_credential() {
    let (node, prov) = node_with(Arc::new(TestProvider::default())).await;
    let admin = principal("user:admin", &[READ]);
    let calls = (0..8).map(|_| {
        let node = node.clone();
        let admin = admin.clone();
        async move { call(&node, &admin, "update.status", json!({})).await }
    });
    for out in futures::future::join_all(calls).await {
        assert_no_credential(&out.expect("status"));
    }
    assert_eq!(
        prov.provisions.load(Ordering::SeqCst),
        1,
        "a concurrent double-trigger must mint exactly one credential"
    );
}

/// A provider with no enrolment handshake degrades to "paste it instead" — `credential.status`
/// answers not-configured and `status` still works. `Unsupported` from `provision_credential` is a
/// normal answer, never an error page.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_provisioning_degrades_to_unconfigured() {
    let (node, _) = node_with(Arc::new(TestProvider {
        provision_error: Some(UpdateError::Unsupported),
        ..Default::default()
    }))
    .await;
    let admin = principal("user:admin", &[READ]);
    let st = call(&node, &admin, "update.status", json!({}))
        .await
        .expect("status still answers");
    assert_eq!(st["supported"], json!(true));
    assert_eq!(st["credential"]["configured"], json!(false));
    assert_eq!(st["credential"]["source"], json!("none"));
}

// ---------------------------------------------------------------------------------------------
// Enrolment: verify-before-seal, and the env fallback
// ---------------------------------------------------------------------------------------------

/// `credential.set` VERIFIES before sealing, and a refused candidate leaves the store untouched — a
/// store write that has not been proven to work is a trap set for the next outage (decision 4).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn set_verifies_before_sealing_and_a_bad_credential_writes_nothing() {
    let (node, _) = node_with(Arc::new(TestProvider {
        // No auto-enrolment in this test: it would seal a credential and mask the "nothing written"
        // assertion below.
        provision_error: Some(UpdateError::Unsupported),
        ..Default::default()
    }))
    .await;
    let admin = principal("user:admin", &[CREDENTIAL, READ]);

    let err = call(
        &node,
        &admin,
        "update.credential.set",
        json!({"value": "WRONG"}),
    )
    .await
    .expect_err("a wrong credential must be refused");
    assert!(
        matches!(&err, ToolError::Extension(m) if m.contains("unauthorized")),
        "the refusal names its cause, got {err:?}"
    );
    assert!(
        lb_store::read(&node.store, WS, "secret", SECRET_PATH)
            .await
            .expect("store read")
            .is_none(),
        "a failed verification must leave the secret plane untouched"
    );

    // The right one verifies, seals, and answers with a fingerprint — never the value.
    let out = call(
        &node,
        &admin,
        "update.credential.set",
        json!({"value": MINTED}),
    )
    .await
    .expect("a verified credential seals");
    assert_no_credential(&out);
    let st: CredentialStatus = serde_json::from_value(out).expect("credential status shape");
    assert!(st.configured);
    assert!(st.fingerprint.is_some());
}

/// A SEALED credential beats the env NAME — resolution order is sealed → env → none, and a node that
/// has been enrolled must not silently fall back to a stale environment value.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sealed_credential_beats_the_env_name() {
    let prov = Arc::new(TestProvider {
        provision_error: Some(UpdateError::Unsupported),
        ..Default::default()
    });
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let mut cfg = UpdateConfig::new(prov.clone());
    cfg.credential_secret = Some(SECRET_PATH.into());
    cfg.credential_env = Some("LB_TEST_UPDATE_CREDENTIAL".into());
    node.install_update(Some(cfg), WS);
    // Safety: this test process sets its own var and no other test reads this name.
    unsafe { std::env::set_var("LB_TEST_UPDATE_CREDENTIAL", "ENV-VALUE") };

    let admin = principal("user:admin", &[READ, CREDENTIAL]);
    let st = call(&node, &admin, "update.credential.status", json!({}))
        .await
        .expect("status");
    assert_eq!(
        st["source"],
        json!("env"),
        "env is the fallback when nothing is sealed"
    );

    call(
        &node,
        &admin,
        "update.credential.set",
        json!({"value": MINTED}),
    )
    .await
    .expect("seal");
    let st = call(&node, &admin, "update.credential.status", json!({}))
        .await
        .expect("status");
    assert_eq!(
        st["source"],
        json!("sealed"),
        "sealed wins over the env NAME"
    );
    assert_eq!(
        prov.last_credential.lock().unwrap().as_deref(),
        None,
        "credential.status does not call the provider"
    );
    unsafe { std::env::remove_var("LB_TEST_UPDATE_CREDENTIAL") };
}

// ---------------------------------------------------------------------------------------------
// apply / rollback / history
// ---------------------------------------------------------------------------------------------

/// `apply` answers **accepted + tx**, never "it worked", and writes an audit row that names the
/// actor — "who replaced the binary on this box" must survive the binary.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_returns_accepted_with_a_tx_and_writes_an_audit_row() {
    let (node, _) = node_with(Arc::new(TestProvider::default())).await;
    let admin = principal("user:admin", &[APPLY]);

    let out = call(&node, &admin, "update.apply", json!({"version": "0.1.2"}))
        .await
        .expect("apply is accepted");
    assert_eq!(out["accepted"], json!(true));
    assert_eq!(out["tx"], json!("tx-apply"));

    let rows = lb_store::scan_all(&node.store, WS, "update_audit")
        .await
        .expect("audit scan");
    let text = serde_json::to_string(&rows).unwrap();
    assert!(
        text.contains("user:admin"),
        "the audit names the actor: {text}"
    );
    assert!(
        text.contains("update.apply"),
        "the audit names the verb: {text}"
    );
    assert!(
        text.contains("tx-apply"),
        "the audit carries the tx: {text}"
    );
    assert!(
        !text.contains(MINTED),
        "the audit must never carry the credential"
    );
}

/// A refusal keeps its reason: an unknown version is a `BadInput` naming the version, and a
/// quarantined one is the provider's `Conflict` — "a bare refusal tells an operator nothing
/// actionable".
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_refusals_reach_the_caller_with_their_reason() {
    let (node, _) = node_with(Arc::new(TestProvider::default())).await;
    let admin = principal("user:admin", &[APPLY]);

    match call(&node, &admin, "update.apply", json!({"version": "9.9.9"})).await {
        Err(ToolError::BadInput(m)) => assert!(m.contains("9.9.9"), "names the version: {m}"),
        other => panic!("expected a BadInput naming the version, got {other:?}"),
    }
    match call(&node, &admin, "update.apply", json!({"version": "0.1.1"})).await {
        Err(ToolError::BadInput(m)) => assert!(m.contains("quarantined"), "keeps the reason: {m}"),
        other => panic!("expected the provider's conflict reason, got {other:?}"),
    }
}

/// `check` returns the PROVIDER's order verbatim and derives nothing lb has no business deriving —
/// lb must not parse, compare, or order version strings.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn check_reports_the_providers_order() {
    let (node, _) = node_with(Arc::new(TestProvider::default())).await;
    let admin = principal("user:admin", &[READ]);
    let out = call(&node, &admin, "update.check", json!({}))
        .await
        .expect("check");
    assert_eq!(out["current"], json!("0.1.0"));
    assert_eq!(out["newest"], json!("0.1.2"));
    assert_eq!(out["update_available"], json!(true));
    assert_eq!(out["available"][0]["source"], json!("remote"));
}

/// `history` is bounded and passes the caller's limit to the provider.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_is_bounded() {
    let (node, _) = node_with(Arc::new(TestProvider::default())).await;
    let admin = principal("user:admin", &[READ]);
    let out = call(&node, &admin, "update.history", json!({"limit": 5}))
        .await
        .expect("history");
    assert_eq!(out["events"][0]["outcome"], json!("committed:limit=5"));
    // An absurd limit is clamped, never forwarded — a caller cannot ask a backend for an unbounded
    // journal.
    let out = call(&node, &admin, "update.history", json!({"limit": 100_000}))
        .await
        .expect("history");
    assert_eq!(out["events"][0]["outcome"], json!("committed:limit=200"));
}

// ---------------------------------------------------------------------------------------------
// The custody wall — from EVERY workspace
// ---------------------------------------------------------------------------------------------

/// The sealed credential is unreadable through `secret.get` (the owner is the host, so every human
/// caller is a non-owner, even holding `secret:*:get`) AND through `store.query` (the raw-read wall)
/// — and from a FOREIGN workspace as well as the boot one. A test that only covers `secret.get`
/// tests the locked door and ignores the window.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_sealed_credential_is_unreadable_on_every_surface_from_every_workspace() {
    let (node, _) = node_with(Arc::new(TestProvider::default())).await;
    // Seal it via the real path.
    let admin = principal("user:admin", &[READ]);
    call(&node, &admin, "update.status", json!({}))
        .await
        .expect("status auto-enrols");

    for ws in [WS, "other-tenant"] {
        let reader = principal_in(
            "user:snoop",
            ws,
            &[
                "mcp:secret.get:call",
                "secret:*:get",
                "mcp:store.query:call",
                "store:*:read",
            ],
        );

        // The door: `secret.get` — denied by the owner wall (owner is `host:update`).
        let out = call(&node, &reader, "secret.get", json!({ "path": SECRET_PATH })).await;
        match out {
            Err(_) => {}
            Ok(v) => assert!(
                !v.to_string().contains(MINTED),
                "secret.get returned the credential in ws={ws}: {v}"
            ),
        }

        // The window: a raw `SELECT` over the secret plane — refused structurally, regardless of caps.
        let out = call(
            &node,
            &reader,
            "store.query",
            json!({ "sql": "SELECT * FROM secret" }),
        )
        .await;
        match out {
            Err(_) => {}
            Ok(v) => panic!("store.query read the secret plane in ws={ws}: {v}"),
        }
    }
}
