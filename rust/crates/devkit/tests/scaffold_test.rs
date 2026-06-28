use std::path::{Path, PathBuf};

use lb_devkit::{
    inspect_extension, resolve_under_root, scaffold_extension, Feature, ScaffoldRequest, Tier,
};

fn temp_root(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("lb-devkit-scaffold-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp root");
    dir
}

fn request(id: &str, tier: Tier) -> ScaffoldRequest {
    ScaffoldRequest {
        id: id.into(),
        tier,
        features: vec![Feature::Ui, Feature::SeriesRead, Feature::Ingest],
    }
}

#[test]
fn scaffolds_wasm_with_manifest_caps_and_ui() {
    let root = temp_root("wasm");
    let report = scaffold_extension(Some(&root), &request("cool-panel", Tier::Wasm)).unwrap();
    assert!(report.path.join("src/lib.rs").is_file());
    assert!(report.path.join("ui/src/mount.tsx").is_file());

    let inspected = inspect_extension(&report.path).unwrap();
    assert_eq!(inspected.id, "cool-panel");
    assert_eq!(inspected.tier, Tier::Wasm);
    assert!(inspected.tools.contains(&"ping".to_string()));
    assert!(inspected
        .caps
        .contains(&"mcp:series.latest:call".to_string()));
    assert!(inspected
        .caps
        .contains(&"mcp:ingest.write:call".to_string()));
}

#[test]
fn scaffolds_native_with_native_recipe() {
    let root = temp_root("native");
    let report = scaffold_extension(Some(&root), &request("native-panel", Tier::Native)).unwrap();
    let manifest = std::fs::read_to_string(report.path.join("extension.toml")).unwrap();

    assert!(report.path.join("src/main.rs").is_file());
    assert!(manifest.contains("tier = \"native\""));
    assert!(manifest.contains("[native]"));
}

#[test]
fn rejects_traversal_before_writing() {
    let root = temp_root("traversal");
    let err = resolve_under_root(&root, Path::new("../escape")).unwrap_err();
    assert!(err.to_string().contains("traversal"));
    assert!(!root.join("../escape").exists());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_escape_before_writing() {
    use std::os::unix::fs::symlink;

    let root = temp_root("symlink-root");
    let outside = temp_root("symlink-outside");
    symlink(&outside, root.join("link")).unwrap();

    let err = resolve_under_root(&root, Path::new("link/escape")).unwrap_err();
    assert!(err.to_string().contains("escapes"));
    assert!(!outside.join("escape").exists());
}
