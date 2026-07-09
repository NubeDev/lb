//! Extension SDK host service tests. Real store, real caps, real job records, and real cargo build
//! for the job path. No fake backend: the only external process is the sanctioned devkit Toolchain.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_devkit::{scaffold_extension, Feature, ScaffoldRequest, Tier};
use lb_host::{
    call_devkit_tool, devkit_build, devkit_scaffold, devkit_templates, DevkitError, Node,
};
use lb_jobs::{load, JobStatus};
use lb_mcp::ToolError;
use serde_json::json;
use std::sync::Arc;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:devkit".into(),
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

fn rust_extensions_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions")
}

fn request(id: &str, tier: Tier) -> ScaffoldRequest {
    ScaffoldRequest {
        id: id.into(),
        tier,
        features: vec![Feature::SeriesRead],
    }
}

#[test]
fn scaffold_without_grant_is_denied_and_writes_nothing() {
    let root = std::env::temp_dir().join(format!("lb-devkit-host-deny-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let denied = principal("devkit-deny", &[]);

    let err = devkit_scaffold(
        &denied,
        "devkit-deny",
        Some(&root),
        &request("denied-ext", Tier::Wasm),
    )
    .unwrap_err();

    assert!(matches!(err, DevkitError::Denied));
    assert!(!root.join("denied-ext").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_devkit_mcp_verb_denies_without_its_grant() {
    let node = Arc::new(Node::boot().await.unwrap());
    let denied = principal("devkit-mcp-deny", &[]);

    let cases = [
        ("devkit.templates", json!({})),
        (
            "devkit.scaffold",
            json!({ "id": "x-deny", "tier": "wasm", "features": [] }),
        ),
        ("devkit.inspect", json!({ "path": "x-deny" })),
        ("devkit.build", json!({ "path": "x-deny", "ts": 1 })),
    ];
    for (tool, input) in cases {
        let err = call_devkit_tool(&node, &denied, "devkit-mcp-deny", tool, &input)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::Denied), "{tool} => {err:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn build_refuses_path_outside_allow_root_before_job_record() {
    let _guard = env_lock().lock().unwrap();
    let node = Arc::new(Node::boot().await.unwrap());
    let root = std::env::temp_dir().join(format!("lb-devkit-allow-{}", std::process::id()));
    let outside = std::env::temp_dir().join(format!("lb-devkit-outside-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::env::set_var("LB_DEVKIT_ROOT", &root);

    let granted = principal("devkit-root-deny", &["mcp:devkit.build:call"]);
    let err = devkit_build(&node, &granted, "devkit-root-deny", &outside, 42)
        .await
        .unwrap_err();

    assert!(matches!(err, DevkitError::BadInput(_)));
    assert!(load(&node.store, "devkit-root-deny", "devkit-build-42")
        .await
        .unwrap()
        .is_none());
    std::env::remove_var("LB_DEVKIT_ROOT");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn build_job_record_is_workspace_scoped() {
    let _guard = env_lock().lock().unwrap();
    let node = Arc::new(Node::boot().await.unwrap());
    let root = rust_extensions_root();
    let id = format!("devkit-host-build-{}", std::process::id());
    let _ = std::fs::remove_dir_all(root.join(&id));
    let report = scaffold_extension(Some(&root), &request(&id, Tier::Native)).unwrap();
    std::env::set_var("LB_DEVKIT_ROOT", &root);

    let granted = principal("devkit-job-a", &["mcp:devkit.build:call"]);
    let started = devkit_build(&node, &granted, "devkit-job-a", &report.path, 77)
        .await
        .expect("build starts");

    for _ in 0..60 {
        if let Some(job) = load(&node.store, "devkit-job-a", &started.job_id)
            .await
            .unwrap()
        {
            if matches!(job.status, JobStatus::Done | JobStatus::Failed) {
                assert_eq!(job.status, JobStatus::Done);
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    assert!(load(&node.store, "devkit-job-b", &started.job_id)
        .await
        .unwrap()
        .is_none());
    let _ = std::fs::remove_dir_all(root.join(&id));
    std::env::remove_var("LB_DEVKIT_ROOT");
}

#[test]
fn templates_requires_grant() {
    let denied = principal("devkit-template-deny", &[]);
    assert!(matches!(
        devkit_templates(&denied, "devkit-template-deny"),
        Err(DevkitError::Denied)
    ));
}
