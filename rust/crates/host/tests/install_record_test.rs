//! Extension install records (extensions scope, README §6.4): `install_extension` persists the
//! `requested ∩ admin_approved` grant set so it survives a restart, and the record is
//! workspace-isolated (mandatory §2.2). This closes the S1 deferral ("admin_approved passed in by
//! the caller") by making the approved set durable per workspace.
//!
//! Booting a Node boots a Zenoh peer → multi-thread flavor + a unique workspace id per test
//! (carry-forward from S3; in-process peers share a workspace's keyspace).

use lb_host::{install_extension, installed, Node};

// A minimal manifest requesting two store caps; the admin approves only one.
const MANIFEST: &str = r#"
[extension]
id = "notes"
version = "0.2.0"
name = "Notes"
description = "test"

[runtime]
tier = "wasm"
world = "lazybones:ext/extension@0.1.0"
placement = "either"

[capabilities]
request = ["store:note:read", "store:note:write"]

[[tools]]
name = "get"
description = "g"

[visibility]
class = "private"
"#;

fn hello_wasm() -> Vec<u8> {
    // Reuse the built hello component — install_extension loads a real component. (The manifest
    // id/tools above are independent of the component's exported tools; this test asserts the
    // PERSISTED grant set, not a tool call.)
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm"
    );
    std::fs::read(path)
        .expect("hello component built (cargo build --target wasm32-wasip2 --release)")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn install_persists_the_approved_grant_intersection() {
    let ws = "ws-install-persist";
    let node = Node::boot().await.unwrap();

    // Admin approves only `store:note:read` (not write). The granted set must be the intersection.
    let approved = vec!["store:note:read".to_string()];
    install_extension(&node, ws, MANIFEST, &hello_wasm(), &approved, 1)
        .await
        .unwrap();

    let rec = installed(&node, ws, "notes")
        .await
        .unwrap()
        .expect("installed");
    assert_eq!(rec.ext_id, "notes");
    assert_eq!(rec.version, "0.2.0");
    assert_eq!(
        rec.granted,
        vec!["store:note:read".to_string()],
        "granted = requested ∩ approved — write was not approved"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn install_record_is_workspace_isolated() {
    let node = Node::boot().await.unwrap();
    install_extension(
        &node,
        "ws-install-a",
        MANIFEST,
        &hello_wasm(),
        &["store:note:read".to_string()],
        1,
    )
    .await
    .unwrap();

    // Another workspace never sees workspace A's install record (README §7).
    assert!(installed(&node, "ws-install-b", "notes")
        .await
        .unwrap()
        .is_none());
    assert!(installed(&node, "ws-install-a", "notes")
        .await
        .unwrap()
        .is_some());
}
