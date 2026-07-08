//! S7 native-tier slice — the SUPERVISION/RESTART category (testing §2, the remaining half of the S7
//! exit gate): "a native sidecar is supervised and restarts cleanly". Proven with a **REAL OS child
//! process** (the reference `echo-sidecar` binary) — the one true external this slice mocks nothing
//! of (testing §3: a real process IS the external). Real embedded SurrealDB + in-proc Zenoh + a real
//! supervised child throughout.
//!
//! The proof, end to end:
//!   1. install the native `echo-sidecar` → it spawns and answers a tool `call` (tagged with the
//!      injected workspace identity — proving the scoped env reached the child);
//!   2. post a channel message (durable STATE) before the crash;
//!   3. trigger a **crash** (the `crash` tool replies then exits the process); the NEXT `call`
//!      detects the dead child, restarts it cleanly (restart_count increments in the durable status),
//!      and ANSWERS — the killed sidecar resumed;
//!   4. assert the channel history is INTACT across the restart — no durable state lost (the child
//!      held none: the stateless-extension guarantee carried into Tier 2);
//!   5. `stop` it cooperatively → durable status reflects Stopped.

use std::collections::HashMap;
use std::path::PathBuf;

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_sidecar, history, install_native, install_native_from_registry, post, reset_native,
    status_native, stop_native, Lifecycle, NativeServiceError, Node, RegistryServiceError, Source,
};
use lb_inbox::Item;
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys};
use lb_supervisor::OsLauncher;

const MANIFEST: &str = include_str!("../../../extensions/echo-sidecar/extension.toml");

fn sidecar_bytes() -> Vec<u8> {
    let path = PathBuf::from(sidecar_dir()).join("echo-sidecar");
    std::fs::read(&path).expect("read echo-sidecar binary")
}

/// The directory holding the built reference sidecar binary. Override with ECHO_SIDECAR_BIN; default
/// to the cargo target. Panics with the build hint (the test-runner "missing component" gotcha, for
/// the native peer of the wasm guest).
fn sidecar_dir() -> String {
    if let Ok(p) = std::env::var("ECHO_SIDECAR_BIN") {
        return PathBuf::from(p)
            .parent()
            .unwrap()
            .to_string_lossy()
            .into_owned();
    }
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug");
    if !dir.join("echo-sidecar").exists() {
        panic!(
            "missing echo-sidecar at {} — run: (cd rust && cargo build -p echo-sidecar)",
            dir.join("echo-sidecar").display()
        );
    }
    dir.to_string_lossy().into_owned()
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn killed_sidecar_restarts_cleanly_with_no_durable_state_lost() {
    let ws = "native-restart";
    let node = Node::boot().await.unwrap();
    let launcher = OsLauncher;
    let admin = principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.stop:call",
            "mcp:native.status:call",
            "bus:chan/general:pub",
            "bus:chan/general:sub",
        ],
    );

    // --- 1. install (spawn) the native sidecar ---
    let supervised = install_native(
        &node,
        &launcher,
        &admin,
        ws,
        MANIFEST,
        &sidecar_dir(),
        &[],
        1,
    )
    .await
    .expect("native sidecar installs + spawns");
    assert_eq!(supervised.version, "0.1.0");
    assert_eq!(supervised.tools, vec!["echo".to_string()]);

    // It answers, tagged with the injected workspace identity (the scoped env reached the child).
    let out = call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "echo",
        r#""hi""#,
        1,
    )
    .await
    .expect("echo answers");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"], "hi");
    assert_eq!(v["ws"], ws, "the injected LB_EXT_WS reached the child");

    // --- 2. durable STATE: a channel message posted before the crash ---
    post(
        &node,
        &admin,
        ws,
        "general",
        Item::new("m0", "general", "user:test", "before-crash", 1),
    )
    .await
    .expect("post");

    // --- 3. CRASH: the `crash` tool replies then exits the child process (deterministic). The call
    //        itself succeeds (the reply landed before the exit) — the child dies AFTER. ---
    call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "crash",
        "null",
        2,
    )
    .await
    .expect("crash tool replied before the child exited");

    // --- 3b. the NEXT call finds the dead child, restarts it cleanly, and answers (the supervision
    //         proof: a killed sidecar is restarted and the call still succeeds). ---
    let after = call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "echo",
        r#""after""#,
        3,
    )
    .await
    .expect("echo answers after the crash+restart");
    let av: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(
        av["echo"], "after",
        "the restarted sidecar resumes answering"
    );
    assert_eq!(av["ws"], ws, "identity re-injected on respawn");

    // restart_count advanced in the durable status (the supervisor restarted the child).
    let status = status_native(&node, &admin, ws, "echo-sidecar")
        .await
        .unwrap()
        .expect("status exists");
    assert_eq!(
        status.restart_count, 1,
        "the killed sidecar was restarted exactly once"
    );
    assert_eq!(status.lifecycle, Lifecycle::Started);

    // --- 4. durable STATE intact across the restart ---
    let bodies: Vec<String> = history(&node.store, &admin, ws, "general")
        .await
        .unwrap()
        .into_iter()
        .map(|i| i.body)
        .collect();
    assert_eq!(
        bodies,
        ["before-crash"],
        "durable channel history must survive the sidecar crash+restart"
    );

    // --- 5. cooperative stop → durable status reflects Stopped ---
    stop_native(&node, &admin, ws, "echo-sidecar", 5)
        .await
        .expect("stops");
    let stopped = status_native(&node, &admin, ws, "echo-sidecar")
        .await
        .unwrap()
        .expect("status exists");
    assert_eq!(stopped.lifecycle, Lifecycle::Stopped);
    assert!(
        !node.sidecars.is_running(ws, "echo-sidecar"),
        "stopped sidecar is no longer live"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn exhausted_budget_is_recovered_by_reset_without_bouncing_the_node() {
    // The resilience proof (native-tier), with a REAL OS child: crash-loop the sidecar past its
    // restart budget so `call` returns "restart budget exhausted" (the permanent dead-end the live
    // bug hit), then `reset` re-arms the budget + respawns and the SAME call answers again — recovery
    // without a node bounce. Default budget is max_restarts=5, so five crash+recover cycles exhaust it.
    let ws = "native-reset";
    let node = Node::boot().await.unwrap();
    let launcher = OsLauncher;
    let admin = principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.reset:call",
            "mcp:native.status:call",
        ],
    );

    install_native(
        &node,
        &launcher,
        &admin,
        ws,
        MANIFEST,
        &sidecar_dir(),
        &[],
        1,
    )
    .await
    .expect("installs + spawns");

    // Exhaust the budget: each crash kills the child, the NEXT call restarts it (bumping the count).
    // After 5 restarts the 6th recovery is refused → the call fails with budget-exhausted.
    let mut ts = 2u64;
    for i in 0..5 {
        // crash tool replies then exits; the call itself succeeds.
        call_sidecar(
            &node,
            &launcher,
            &admin,
            ws,
            "echo-sidecar",
            "crash",
            "null",
            ts,
        )
        .await
        .unwrap_or_else(|e| panic!("crash {i} tool replied: {e:?}"));
        ts += 1;
        // next call restarts the dead child and answers (restarts 1..=5).
        call_sidecar(
            &node,
            &launcher,
            &admin,
            ws,
            "echo-sidecar",
            "echo",
            r#""x""#,
            ts,
        )
        .await
        .unwrap_or_else(|e| panic!("recover after crash {i}: {e:?}"));
        ts += 1;
    }
    let status = status_native(&node, &admin, ws, "echo-sidecar")
        .await
        .unwrap()
        .expect("status");
    assert_eq!(status.restart_count, 5, "budget spent (max_restarts=5)");

    // The 6th crash exhausts the budget: the recovery call now fails with the exhausted error — the
    // permanent dead-end (every subsequent call would fail until the node is bounced).
    call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "crash",
        "null",
        ts,
    )
    .await
    .expect("6th crash tool replied before exit");
    ts += 1;
    let err = call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "echo",
        r#""dead""#,
        ts,
    )
    .await
    .expect_err("budget is exhausted → the call cannot recover the child");
    assert!(
        matches!(&err, NativeServiceError::Supervisor(s)
            if matches!(s, lb_supervisor::SupervisorError::RestartExhausted(_))),
        "exhausted budget surfaces as RestartExhausted, not a silent hang: {err:?}"
    );
    ts += 1;

    // --- RESET: re-arm the budget + force a fresh child (the operator rescue) ---
    reset_native(&node, &launcher, &admin, ws, "echo-sidecar", ts)
        .await
        .expect("reset re-arms the exhausted sidecar without a node bounce");
    ts += 1;

    // The SAME call answers again — no longer a dead end — and the durable count is re-armed to 0.
    let out = call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "echo",
        r#""alive""#,
        ts,
    )
    .await
    .expect("the reset sidecar answers again");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"], "alive", "recovered sidecar resumes serving");
    let after = status_native(&node, &admin, ws, "echo-sidecar")
        .await
        .unwrap()
        .expect("status");
    assert_eq!(
        after.restart_count, 0,
        "reset zeroed the durable restart count"
    );
    assert_eq!(after.lifecycle, Lifecycle::Started);

    stop_native(&node, &admin, ws, "echo-sidecar", ts + 1)
        .await
        .ok();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn sustained_health_decays_the_restart_count() {
    // Auto-decay (native-tier resilience): a sidecar that crashed once but then serves calls cleanly
    // for the cool-off window has its restart count decayed back to zero — a TRANSIENT crash no longer
    // permanently erodes the budget. Real OS child; we drive the cool-off by advancing the injected
    // logical clock past the default 30s window (30_000ms) rather than sleeping.
    let ws = "native-decay";
    let node = Node::boot().await.unwrap();
    let launcher = OsLauncher;
    let admin = principal(
        ws,
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.status:call",
        ],
    );

    install_native(
        &node,
        &launcher,
        &admin,
        ws,
        MANIFEST,
        &sidecar_dir(),
        &[],
        1_000,
    )
    .await
    .expect("installs");

    // One crash → one restart. healthy_since is reset to the restart ts (2_000).
    call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "crash",
        "null",
        2_000,
    )
    .await
    .expect("crash replied");
    call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "echo",
        r#""r""#,
        2_000,
    )
    .await
    .expect("recover");
    let after_crash = status_native(&node, &admin, ws, "echo-sidecar")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_crash.restart_count, 1, "one restart recorded");

    // A call WITHIN the cool-off window (now - 2_000 < 30_000) must NOT decay the count.
    call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "echo",
        r#""soon""#,
        10_000,
    )
    .await
    .expect("call within cooloff");
    assert_eq!(
        status_native(&node, &admin, ws, "echo-sidecar")
            .await
            .unwrap()
            .unwrap()
            .restart_count,
        1,
        "count must not decay before the cool-off window elapses"
    );

    // A successful call AFTER the cool-off window (2_000 + 30_000 = 32_000) decays the count to 0.
    call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "echo",
        r#""later""#,
        33_000,
    )
    .await
    .expect("call after cooloff");
    assert_eq!(
        status_native(&node, &admin, ws, "echo-sidecar")
            .await
            .unwrap()
            .unwrap()
            .restart_count,
        0,
        "sustained health decays the restart count — a transient crash is not permanent"
    );

    stop_native(&node, &admin, ws, "echo-sidecar", 40_000)
        .await
        .ok();
}

// ---- registry × native composition: a signed native artifact installs through the registry ----

struct MapSource(HashMap<(String, String), Artifact>);
impl Source for MapSource {
    async fn fetch(&self, ext_id: &str, version: &str) -> Result<Artifact, RegistryServiceError> {
        self.0
            .get(&(ext_id.to_string(), version.to_string()))
            .cloned()
            .ok_or_else(|| RegistryServiceError::NotAvailable(format!("{ext_id}@{version}")))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn native_artifact_installs_through_registry() {
    // The two S7 slices compose: a SIGNED tier="native" artifact pulls→verifies→writes the binary→
    // is supervised. Both gates hold — the signature gate (in pull) and the capability gate (in the
    // native install). Proven with the real echo-sidecar binary as the artifact bytes.
    let ws = "native-registry";
    let node = Node::boot().await.unwrap();
    let launcher = OsLauncher;

    // A publisher key, and the signed native artifact (manifest ‖ binary bound by the digest).
    let sk = PublisherSigningKey::from_bytes(&[40u8; 32]);
    let kid = "pub-native".to_string();
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    let trusted = TrustedKeys::from([(kid.clone(), pk)]);

    let bytes = sidecar_bytes();
    let d = digest(MANIFEST, &bytes);
    let artifact = Artifact {
        ext_id: "echo-sidecar".into(),
        version: "0.1.0".into(),
        manifest_toml: MANIFEST.into(),
        wasm: bytes,
        digest_hex: digest_hex(&d),
        publisher_key_id: kid.clone(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    };
    let source = MapSource(
        [(("echo-sidecar".to_string(), "0.1.0".to_string()), artifact)]
            .into_iter()
            .collect(),
    );

    // A temp install dir the verified binary lands in.
    let dir = std::env::temp_dir().join(format!("lb-native-{ws}"));
    let dir = dir.to_string_lossy().into_owned();

    let admin = principal(ws, &["mcp:native.install:call", "mcp:native.call:call"]);

    let supervised = install_native_from_registry(
        &node,
        &source,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "0.1.0",
        &dir,
        &trusted,
        &[],
        1,
    )
    .await
    .expect("signed native artifact installs through the registry");
    assert_eq!(supervised.version, "0.1.0");

    // It is supervised and answers — the pulled+verified binary is live.
    let out = call_sidecar(
        &node,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "echo",
        r#""reg""#,
        1,
    )
    .await
    .expect("the registry-installed native sidecar answers");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["echo"], "reg");

    // The signature gate is independent: a TAMPERED native artifact is rejected before any disk
    // write or spawn, even with the install grant.
    let tampered = {
        let mut bytes = sidecar_bytes();
        bytes.push(0xFF); // flip the binary; the digest no longer matches the signature
        let d = digest(MANIFEST, &bytes);
        Artifact {
            ext_id: "echo-sidecar".into(),
            version: "9.9.9".into(),
            manifest_toml: MANIFEST.into(),
            wasm: bytes,
            digest_hex: digest_hex(&d),
            publisher_key_id: kid.clone(),
            // sign a DIFFERENT digest so verification fails (tamper after signing)
            signature: sk.sign(&digest(MANIFEST, b"other")).to_bytes().to_vec(),
        }
    };
    let bad_source = MapSource(
        [(("echo-sidecar".to_string(), "9.9.9".to_string()), tampered)]
            .into_iter()
            .collect(),
    );
    let err = install_native_from_registry(
        &node,
        &bad_source,
        &launcher,
        &admin,
        ws,
        "echo-sidecar",
        "9.9.9",
        &dir,
        &trusted,
        &[],
        2,
    )
    .await
    .expect_err("a tampered native artifact must be rejected");
    assert!(
        matches!(err, RegistryServiceError::Unverified),
        "tampered native artifact rejected by the signature gate: {err:?}"
    );

    // cleanup
    stop_native(
        &node,
        &principal(ws, &["mcp:native.stop:call"]),
        ws,
        "echo-sidecar",
        3,
    )
    .await
    .ok();
    let _ = std::fs::remove_dir_all(&dir);
}
