use std::path::Path;

use lb_devkit::{resolve_under_root, scaffold_extension, write_file, Feature, ScaffoldRequest, Tier};

fn temp_root(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lb-devkit-write-file-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp root");
    dir
}

fn request(id: &str) -> ScaffoldRequest {
    ScaffoldRequest {
        id: id.into(),
        tier: Tier::Wasm,
        features: vec![Feature::Ui],
    }
}

#[test]
fn writes_a_new_file_under_the_extension_dir() {
    let root = temp_root("new");
    let report = scaffold_extension(Some(&root), &request("writer-a")).unwrap();
    // An agent authors a custom widget file the scaffold did not produce.
    let target = Path::new("writer-a/src/widgets/Energy.tsx");
    let body = "// energy dashboard widget\n";
    let out = write_file(Some(&root), target, body).unwrap();
    assert_eq!(out.bytes, body.len() as u64);
    let on_disk =
        std::fs::read_to_string(report.path.join("src/widgets/Energy.tsx")).unwrap();
    assert_eq!(on_disk, "// energy dashboard widget\n");
}

#[test]
fn overwrites_an_existing_file() {
    // The default scaffold produces ui/src/App.tsx; the agent replaces it with a real page.
    let root = temp_root("overwrite");
    scaffold_extension(Some(&root), &request("writer-b")).unwrap();
    let target = Path::new("writer-b/ui/src/App.tsx");
    let body = "export function App() { return <div>real energy page</div>; }\n";
    let out = write_file(Some(&root), target, body).unwrap();
    assert_eq!(out.bytes, body.len() as u64);
    let on_disk = std::fs::read_to_string(root.join("writer-b/ui/src/App.tsx")).unwrap();
    assert!(on_disk.contains("real energy page"));
    // The scaffolded marker is gone — the file was replaced, not appended.
    assert!(!on_disk.contains("Replace this with your content"));
}

#[test]
fn rejects_path_outside_the_devkit_root() {
    // The same traversal guard resolve_under_root uses applies — a hand-built `..` path is refused
    // before any byte is written. This is the safety floor: write_file can't escape the devkit root.
    let root = temp_root("escape");
    scaffold_extension(Some(&root), &request("writer-c")).unwrap();
    let err = write_file(
        Some(&root),
        Path::new("../escape.tsx"),
        "x",
    )
    .unwrap_err();
    assert!(err.to_string().contains("traversal") || err.to_string().contains("escapes"));
    assert!(!root.join("../escape.tsx").exists());
}

#[test]
fn rejects_absolute_path_outside_the_devkit_root() {
    // An absolute path that does not live under the devkit root is refused (the parent must canonicalize
    // to a subdir of the root). This stops an agent from writing to /etc or anywhere outside its workdir.
    let root = temp_root("absolute");
    scaffold_extension(Some(&root), &request("writer-d")).unwrap();
    let outside = std::env::temp_dir().join(format!(
        "lb-devkit-write-file-outside-{}",
        std::process::id()
    ));
    let err = write_file(Some(&root), &outside, "x").unwrap_err();
    assert!(err.to_string().contains("escapes"));
    assert!(!outside.exists());
}

#[test]
fn round_trips_with_resolve_under_root() {
    // The path write_file returns is the canonical form resolve_under_root produces — the same
    // anchor build/inspect use. So a follow-up `devkit.inspect { path }` on the scaffolded dir
    // sees the written file as a normal source file.
    let root = temp_root("roundtrip");
    scaffold_extension(Some(&root), &request("writer-e")).unwrap();
    let out = write_file(
        Some(&root),
        Path::new("writer-e/src/lib.rs"),
        "// replaced\n",
    )
    .unwrap();
    let re_resolved = resolve_under_root(&root, Path::new("writer-e/src/lib.rs")).unwrap();
    assert_eq!(out.path, re_resolved);
}
