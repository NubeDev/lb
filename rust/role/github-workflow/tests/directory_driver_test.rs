//! The **directory-backed** driver — the headline of the dynamic-workspace slice: a workspace
//! registered into the directory is picked up by the **next tick** with no restart, and a deregistered
//! one is dropped. `drive_directory_once` re-reads `enabled_workspaces` each tick and builds bindings,
//! minting each service principal via an injected closure. Real store + Zenoh (a `Node` is booted);
//! the GitHub sink is a recording `Target`.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    deregister_workspace, register_workspace, request_approval, resolve_approval, Node, PrSpec,
    Target,
};
use lb_inbox::Decision;
use lb_outbox::Effect;
use lb_role_github_workflow::drive_directory_once;

/// A service principal for `ws` holding the workflow caps — what the binary's `principal_for` mints.
fn service_principal(ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "ext:coding-workflow".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec![
            "mcp:workflow.request_approval:call".into(),
            "mcp:workflow.resolve_approval:call".into(),
            "mcp:workflow.start_job:call".into(),
            "bus:chan/*:pub".into(),
        ],
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

#[derive(Default)]
struct RecordingTarget {
    keys: std::sync::Mutex<Vec<String>>,
}
impl Target for RecordingTarget {
    async fn deliver(&self, effect: &Effect) -> Result<(), String> {
        self.keys
            .lock()
            .unwrap()
            .push(effect.idempotency_key.clone());
        Ok(())
    }
}

/// Request + approve a coding job in `ws` (the state a tick reacts to).
async fn approve(node: &Node, ws: &str, approval_id: &str) {
    let p = service_principal(ws);
    let pr = PrSpec::new("acme/api", "fix", "main", "Fix it", "");
    request_approval(&node.store, &p, ws, approval_id, "scope", "rev", &pr, 1)
        .await
        .unwrap();
    resolve_approval(&node.store, &p, ws, approval_id, Decision::Approved, 2)
        .await
        .unwrap();
}

fn no_errors(ws: &str, e: String) {
    panic!("unexpected driver error in {ws}: {e}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_workspace_registered_at_runtime_is_picked_up_next_tick() {
    // THE HEADLINE: no restart. A tick over an empty directory does nothing; after `register_workspace`
    // the very next tick services that workspace — the driver re-reads the directory each tick.
    let node = Arc::new(Node::boot().await.unwrap());
    let target = RecordingTarget::default();

    // ws-late has an approved job, but is NOT yet in the directory → the first tick ignores it.
    approve(&node, "dir-late", "ap1").await;
    let t0 = drive_directory_once(&node, &target, 10, service_principal, no_errors).await;
    assert_eq!(
        (t0.started, t0.delivered),
        (0, 0),
        "empty directory → nothing serviced"
    );

    // Operator registers it at runtime.
    register_workspace(&node.store, "dir-late", "progress", 11)
        .await
        .unwrap();

    // The next tick picks it up — the job starts and the PR delivers, no restart.
    let t1 = drive_directory_once(&node, &target, 12, service_principal, no_errors).await;
    assert_eq!(
        (t1.started, t1.delivered),
        (1, 1),
        "the newly-registered ws is serviced"
    );
    assert_eq!(
        target.keys.lock().unwrap().as_slice(),
        &["pr:ap1".to_string()]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_deregistered_workspace_is_dropped_next_tick() {
    // The other direction: a registered ws is serviced; once deregistered, the next tick ignores it.
    let node = Arc::new(Node::boot().await.unwrap());
    let target = RecordingTarget::default();
    register_workspace(&node.store, "dir-drop", "progress", 1)
        .await
        .unwrap();

    // Deregister BEFORE approving — the tick must not service a disabled ws even with work waiting.
    deregister_workspace(&node.store, "dir-drop", 2)
        .await
        .unwrap();
    approve(&node, "dir-drop", "ap1").await;

    let tick = drive_directory_once(&node, &target, 10, service_principal, no_errors).await;
    assert_eq!(
        (tick.started, tick.delivered),
        (0, 0),
        "a deregistered ws is not serviced"
    );
    assert!(
        lb_jobs::load(&node.store, "dir-drop", "job:ap1")
            .await
            .unwrap()
            .is_none(),
        "no job started for the deregistered ws"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_directory_driver_isolates_workspaces() {
    // MANDATORY workspace-isolation (§2.2): two registered workspaces, each with an approved job — a
    // tick services each in ITS namespace; neither job/effect crosses. (The directory holds many; the
    // per-binding calls each select their own ws.)
    let node = Arc::new(Node::boot().await.unwrap());
    let target = RecordingTarget::default();
    register_workspace(&node.store, "dir-iso-a", "progress", 1)
        .await
        .unwrap();
    register_workspace(&node.store, "dir-iso-b", "progress", 2)
        .await
        .unwrap();
    approve(&node, "dir-iso-a", "apA").await;
    approve(&node, "dir-iso-b", "apB").await;

    let tick = drive_directory_once(&node, &target, 10, service_principal, no_errors).await;
    assert_eq!(
        (tick.started, tick.delivered),
        (2, 2),
        "both serviced, each in its own ws"
    );

    // Each ws started its OWN job under the deterministic id — never the other's.
    assert!(lb_jobs::load(&node.store, "dir-iso-a", "job:apA")
        .await
        .unwrap()
        .is_some());
    assert!(lb_jobs::load(&node.store, "dir-iso-b", "job:apB")
        .await
        .unwrap()
        .is_some());
    assert!(
        lb_jobs::load(&node.store, "dir-iso-a", "job:apB")
            .await
            .unwrap()
            .is_none(),
        "ws-A never started ws-B's job"
    );

    let mut keys = target.keys.lock().unwrap().clone();
    keys.sort();
    assert_eq!(keys, vec!["pr:apA".to_string(), "pr:apB".to_string()]);
}
