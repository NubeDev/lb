// inspect_extension must report the concrete build outputs (size + mtime) on disk, and derive
// `built` from a real compiled binary — not merely a `release/` dir. We seed a real scaffold, then
// write real files where cargo/vite would land them (no mocks; rule 9) and assert the report.

use std::fs;
use std::path::PathBuf;

use lb_devkit::{inspect_extension, scaffold_extension, Feature, ScaffoldRequest, Tier};

fn ext_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("rust root")
        .join("extensions")
}

fn scaffold_id(name: &str) -> String {
    format!("devkit-inspect-{name}-{}", std::process::id())
}

fn scaffold(name: &str, tier: Tier) -> PathBuf {
    let root = ext_root();
    let _ = fs::remove_dir_all(root.join(scaffold_id(name)));
    let req = ScaffoldRequest {
        id: scaffold_id(name),
        tier,
        features: vec![Feature::Ui, Feature::SeriesRead],
    };
    scaffold_extension(Some(&root), &req).unwrap().path
}

#[test]
fn reports_wasm_and_remote_entry_artifacts_with_sizes() {
    let path = scaffold("wasm", Tier::Wasm);

    // Before any build: no binary, so not built and no wasm/native artifact.
    let pre = inspect_extension(&path).unwrap();
    assert!(!pre.built, "should not be built before any artifact exists");
    assert!(!pre.artifacts.iter().any(|a| a.kind == "wasm"));

    // Seed the two artifacts a real wasm build produces.
    let release = path.join("target/wasm32-wasip2/release");
    fs::create_dir_all(&release).unwrap();
    fs::write(release.join("thing_ext.wasm"), vec![0u8; 2048]).unwrap();
    let dist = path.join("ui/dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("remoteEntry.js"), b"export const x = 1;\n").unwrap();

    let post = inspect_extension(&path).unwrap();
    let _ = fs::remove_dir_all(&path);

    assert!(post.built, "a wasm component on disk means built");
    let wasm = post
        .artifacts
        .iter()
        .find(|a| a.kind == "wasm")
        .expect("wasm artifact reported");
    assert_eq!(wasm.size, 2048);
    assert!(wasm.mtime.is_some(), "mtime present for a real file");

    let remote = post
        .artifacts
        .iter()
        .find(|a| a.kind == "remote-entry")
        .expect("remote-entry reported");
    assert_eq!(remote.size, 20);
}

#[test]
fn ui_remote_alone_is_not_built() {
    // A federated bundle with no compiled component must NOT read as built — the extension can't
    // load without its binary, and the UI must not tell the user the build succeeded.
    let path = scaffold("uionly", Tier::Native);
    let dist = path.join("ui/dist");
    fs::create_dir_all(&dist).unwrap();
    fs::write(dist.join("remoteEntry.js"), b"x").unwrap();

    let report = inspect_extension(&path).unwrap();
    let _ = fs::remove_dir_all(&path);

    assert!(!report.built, "remote-entry without a binary is not built");
    assert!(report.artifacts.iter().any(|a| a.kind == "remote-entry"));
}
