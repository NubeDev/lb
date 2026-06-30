use std::path::PathBuf;

use lb_devkit::{
    build_extension, scaffold_extension, Feature, ProcessToolchain, ScaffoldRequest, Tier,
    Toolchain,
};

fn temp_root(name: &str) -> PathBuf {
    let rust_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("rust root")
        .to_path_buf();
    let dir = rust_root.join("extensions");
    let _ = std::fs::remove_dir_all(dir.join(scaffold_id(name)));
    dir
}

fn scaffold_id(name: &str) -> String {
    format!("devkit-test-{name}-{}", std::process::id())
}

fn request(name: &str, tier: Tier) -> ScaffoldRequest {
    ScaffoldRequest {
        id: scaffold_id(name),
        tier,
        // Keep this test on backend templates only. UI is exercised by the Studio gateway path; this
        // test's job is proving generated Rust code compiles through the real cargo toolchain.
        features: vec![Feature::SeriesRead],
    }
}

#[test]
fn builds_generated_native_with_real_cargo() {
    let root = temp_root("native");
    let report = scaffold_extension(Some(&root), &request("native", Tier::Native)).unwrap();
    let mut logs = Vec::new();
    let result = build_extension(&report.path, &ProcessToolchain, &mut |line| logs.push(line));
    let _ = std::fs::remove_dir_all(&report.path);
    if let Err(err) = result {
        panic!("build failed: {err}\n{}", logs.join("\n"));
    }
    assert!(
        logs.iter()
            .any(|line| line.contains("cargo build --release")),
        "expected cargo log line, got {logs:?}"
    );
}

#[test]
fn builds_generated_wasm_with_real_cargo_when_target_is_available() {
    if !ProcessToolchain.wasm_target_ready() {
        eprintln!("skipping: wasm32-wasip2 target is not installed");
        return;
    }
    let root = temp_root("wasm");
    let report = scaffold_extension(Some(&root), &request("wasm", Tier::Wasm)).unwrap();
    let mut logs = Vec::new();
    let result = build_extension(&report.path, &ProcessToolchain, &mut |line| logs.push(line));
    let _ = std::fs::remove_dir_all(&report.path);
    if let Err(err) = result {
        panic!("build failed: {err}\n{}", logs.join("\n"));
    }
    assert!(
        logs.iter().any(|line| line.contains("wasm32-wasip2")),
        "expected wasm target log line, got {logs:?}"
    );
}
