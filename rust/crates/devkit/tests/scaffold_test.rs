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
fn ui_template_emits_themed_files() {
    // S3 (external-agent-authoring): the `ui` feature emits a correct-by-construction themed
    // template — Tailwind v4, scoped CSS tokens, a recharts sample chart, no preflight. An agent
    // that fills in real data produces an on-theme page without touching CSS.
    let root = temp_root("themed");
    let report = scaffold_extension(Some(&root), &request("energy-dashboard", Tier::Wasm)).unwrap();
    // The themed CSS files exist.
    let tokens = std::fs::read_to_string(report.path.join("ui/src/styles/tokens.css")).unwrap();
    let main_css = std::fs::read_to_string(report.path.join("ui/src/styles/main.css")).unwrap();
    // The tokens are scoped under `.lbx-<id>` — never `:root`.
    assert!(
        tokens.contains(".lbx-energy-dashboard"),
        "tokens scoped under root class"
    );
    // Never a bare `:root {` selector (the federated-CSS leak). Comments mentioning `:root` are fine.
    assert!(
        !tokens.contains(":root {"),
        "never a :root selector (federated-CSS leak prevention)"
    );
    // The chart ramp aliases the host's --chart-N vars.
    assert!(tokens.contains("var(--chart-1"), "chart-1 alias present");
    // No preflight: main.css imports theme + utilities, NOT the full tailwindcss.
    assert!(main_css.contains("tailwindcss/theme"), "theme imported");
    assert!(
        main_css.contains("tailwindcss/utilities"),
        "utilities imported"
    );
    assert!(
        !main_css.contains("@import \"tailwindcss\";"),
        "no bare `@import tailwindcss` (that includes preflight)"
    );
    // The sample chart reads the scoped chart token.
    let chart =
        std::fs::read_to_string(report.path.join("ui/src/widgets/SampleChart.tsx")).unwrap();
    assert!(
        chart.contains("var(--lbx-chart-1)"),
        "chart series colored from the scoped chart-1"
    );
    assert!(chart.contains("recharts"), "recharts is the chart lib");
    // The package.json carries the themed deps.
    let pkg = std::fs::read_to_string(report.path.join("ui/package.json")).unwrap();
    assert!(pkg.contains("tailwindcss"), "tailwindcss dep present");
    assert!(
        pkg.contains("@tailwindcss/vite"),
        "tailwindcss vite plugin dep present"
    );
    assert!(pkg.contains("recharts"), "recharts dep present");
    // The vite config wires the tailwindcss plugin.
    let vite = std::fs::read_to_string(report.path.join("ui/vite.config.ts")).unwrap();
    assert!(
        vite.contains("tailwindcss()"),
        "tailwindcss plugin wired in vite config"
    );
    // The mount wraps the app in the root class.
    let mount = std::fs::read_to_string(report.path.join("ui/src/mount.tsx")).unwrap();
    assert!(
        mount.contains("lbx-energy-dashboard"),
        "root class wrapper in mount"
    );
    assert!(mount.contains("main.css"), "CSS entry imported in mount");
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

#[test]
fn rejects_ids_reusing_the_build_job_namespace() {
    let root = temp_root("reserved");
    for reserved in ["devkit-build", "devkit-build-native-1047202"] {
        let err = scaffold_extension(Some(&root), &request(reserved, Tier::Native)).unwrap_err();
        assert!(
            err.to_string().contains("reserved"),
            "{reserved} should be rejected: {err}"
        );
        assert!(!root.join(reserved).exists());
    }
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

#[test]
fn scaffold_tolerates_features_as_a_stringified_json_array() {
    // Models frequently pass `features` as a string `"[\"ui\", \"series-read\"]"` instead of a real
    // array. The tolerant deserializer accepts both. This is the bug the agent hit live.
    let root = temp_root("stringified");
    let json = r#"{"id":"str-features","tier":"wasm","features":"[\"ui\", \"series-read\"]"}"#;
    let req: lb_devkit::ScaffoldRequest = serde_json::from_str(json).expect("tolerant parse");
    assert_eq!(req.features.len(), 2);
    assert!(req.features.contains(&Feature::Ui));
    assert!(req.features.contains(&Feature::SeriesRead));
    let report = scaffold_extension(Some(&root), &req).unwrap();
    assert!(report.path.join("ui/src/App.tsx").is_file());
}

#[test]
fn scaffold_tolerates_missing_features() {
    // Absent features ⇒ empty vec (not an error). The agent sometimes omits the field.
    let root = temp_root("no-features");
    let json = r#"{"id":"no-feat","tier":"wasm"}"#;
    let req: lb_devkit::ScaffoldRequest = serde_json::from_str(json).expect("absent features → empty");
    assert!(req.features.is_empty());
    let report = scaffold_extension(Some(&root), &req).unwrap();
    assert!(report.path.join("src/lib.rs").is_file());
}

#[test]
fn scaffold_with_datasources_feature_grants_federation_caps() {
    // The `datasources` feature grants federation.query/schema + datasource.list — the "connect
    // to the server" cap an extension needs to query an external datasource.
    let root = temp_root("datasources");
    let req = ScaffoldRequest {
        id: "ds-panel".into(),
        tier: Tier::Wasm,
        features: vec![Feature::Ui, Feature::Datasources],
    };
    let report = scaffold_extension(Some(&root), &req).unwrap();
    let manifest = std::fs::read_to_string(report.path.join("extension.toml")).unwrap();
    assert!(manifest.contains("mcp:federation.query:call"), "federation.query cap granted");
    assert!(manifest.contains("mcp:federation.schema:call"), "federation.schema cap granted");
    assert!(manifest.contains("mcp:datasource.list:call"), "datasource.list cap granted");
}
