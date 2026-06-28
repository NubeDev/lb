//! Load-bearing extension SDK tests: generated templates are not placeholders. Each case scaffolds,
//! builds with the real toolchain, signs with the shared devkit artifact signer, publishes through the
//! real host path, and calls the installed extension.

use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use lb_auth::{mint, verify, Claims, Principal, Role};
use lb_devkit::{
    build_extension, scaffold_extension, sign_artifact, Feature, ScaffoldRequest, Tier,
};
use lb_host::{call_sidecar, call_tool, ext_publish, stop_native, Node};
use lb_registry::{PublisherKey, TrustedKeys, Visibility};
use lb_supervisor::OsLauncher;

fn principal(ws: &str, caps: &[String]) -> Principal {
    let key = lb_auth::SigningKey::generate();
    let claims = Claims {
        sub: "user:devkit-e2e".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.to_vec(),
        iat: 0,
        exp: u64::MAX,
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

fn publisher() -> (String, SigningKey, TrustedKeys) {
    let key_id = "devkit-e2e".to_string();
    let signing_key = SigningKey::from_bytes(&[7; 32]);
    let publisher =
        PublisherKey::from_bytes(&signing_key.verifying_key().to_bytes()).expect("publisher key");
    (
        key_id.clone(),
        signing_key,
        TrustedKeys::from([(key_id, publisher)]),
    )
}

fn built_bytes(path: &Path, id: &str, tier: Tier) -> Vec<u8> {
    let file = match tier {
        Tier::Wasm => path
            .join("target/wasm32-wasip2/release")
            .join(format!("{}_ext.wasm", id.replace('-', "_"))),
        Tier::Native => path.join("target/release").join(id),
    };
    std::fs::read(&file).unwrap_or_else(|e| panic!("read build output {}: {e}", file.display()))
}

async fn publish_generated(node: &Node, ws: &str, id: &str, tier: Tier) -> PathBuf {
    let root = rust_extensions_root();
    let path = root.join(id);
    let _ = std::fs::remove_dir_all(&path);
    let report = scaffold_extension(Some(&root), &request(id, tier)).expect("scaffold");
    build_extension(
        Path::new(&report.path),
        &lb_devkit::ProcessToolchain,
        &mut |_line| {},
    )
    .expect("build");

    let manifest = std::fs::read_to_string(Path::new(&report.path).join("extension.toml")).unwrap();
    let bytes = built_bytes(Path::new(&report.path), id, tier);
    let (key_id, signing_key, trusted) = publisher();
    let artifact = sign_artifact(manifest, bytes, &key_id, &signing_key).expect("sign artifact");
    let mut caps = vec!["mcp:ext.publish:call".to_string()];
    if tier == Tier::Native {
        caps.push("mcp:native.install:call".to_string());
    }
    let admin = principal(ws, &caps);
    ext_publish(node, &admin, ws, artifact, &trusted, Visibility::Private, 1)
        .await
        .expect("publish");
    PathBuf::from(report.path)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scaffold_build_publish_wasm_then_call_tool() {
    let node = Node::boot().await.unwrap();
    let ws = "devkit-e2e-wasm";
    let id = format!("devkit-e2e-wasm-{}", std::process::id());
    let path = publish_generated(&node, ws, &id, Tier::Wasm).await;

    let caller = principal(ws, &[format!("mcp:{id}.ping:call")]);
    let out = call_tool(
        &std::sync::Arc::new(node),
        &caller,
        ws,
        &format!("{id}.ping"),
        "{}",
    )
    .await
    .expect("call generated wasm");
    assert!(out.contains(r#""tier":"wasm""#), "{out}");
    let _ = std::fs::remove_dir_all(path);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scaffold_build_publish_native_then_call_sidecar() {
    let node = Node::boot().await.unwrap();
    let ws = "devkit-e2e-native";
    let id = format!("devkit-e2e-native-{}", std::process::id());
    let path = publish_generated(&node, ws, &id, Tier::Native).await;

    let caller = principal(ws, &["mcp:native.call:call".to_string()]);
    let out = call_sidecar(&node, &OsLauncher, &caller, ws, &id, "ping", "{}", 2)
        .await
        .expect("call generated native");
    assert!(out.contains(r#""tier":"native""#), "{out}");
    let stopper = principal(ws, &["mcp:native.stop:call".to_string()]);
    let _ = stop_native(&node, &stopper, ws, &id, 3).await;
    let _ = std::fs::remove_dir_all(path);
}
