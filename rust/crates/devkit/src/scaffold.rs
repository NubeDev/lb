use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::feature_caps;
use crate::root::{default_devkit_root, resolve_under_root};
use crate::template::WORLD;
use crate::{Feature, ScaffoldReport, ScaffoldRequest, Tier};

pub fn scaffold_extension(
    root: Option<&Path>,
    request: &ScaffoldRequest,
) -> Result<ScaffoldReport> {
    validate_id(&request.id)?;
    let root = root.map(PathBuf::from).unwrap_or_else(default_devkit_root);
    let target = resolve_under_root(root, &request.id)?;
    if target.exists() {
        bail!("target already exists: {}", target.display());
    }
    fs::create_dir_all(&target).with_context(|| format!("create {}", target.display()))?;

    let mut files = Vec::new();
    for rendered in render_files(request)? {
        let path = target.join(&rendered.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        fs::write(&path, rendered.body).with_context(|| format!("write {}", path.display()))?;
        make_executable_if_script(&path)?;
        files.push(path);
    }
    Ok(ScaffoldReport {
        path: target,
        files,
    })
}

fn make_executable_if_script(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        if path.file_name().and_then(|n| n.to_str()) == Some("build.sh") {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(path, perms)?;
        }
    }
    Ok(())
}

struct RenderedFile {
    path: PathBuf,
    body: String,
}

fn render_files(request: &ScaffoldRequest) -> Result<Vec<RenderedFile>> {
    let mut files = match request.tier {
        Tier::Wasm => wasm_files(request),
        Tier::Native => native_files(request),
    };
    if request.features.contains(&Feature::Ui) {
        files.extend(ui_files(request));
    }
    Ok(files)
}

fn wasm_files(request: &ScaffoldRequest) -> Vec<RenderedFile> {
    vec![
        file("Cargo.toml", render(CARGO_WASM, request)),
        file("extension.toml", render_manifest(request)),
        file("build.sh", render(BUILD_SH, request)),
        file("src/lib.rs", render(WASM_LIB, request)),
    ]
}

fn native_files(request: &ScaffoldRequest) -> Vec<RenderedFile> {
    vec![
        file("Cargo.toml", render(CARGO_NATIVE, request)),
        file("extension.toml", render_manifest(request)),
        file("build.sh", render(BUILD_SH, request)),
        file("src/main.rs", render(NATIVE_MAIN, request)),
    ]
}

fn ui_files(request: &ScaffoldRequest) -> Vec<RenderedFile> {
    vec![
        file("ui/package.json", render(UI_PACKAGE, request)),
        file("ui/index.html", render(UI_INDEX, request)),
        file("ui/src/App.tsx", render(UI_APP, request)),
        file("ui/src/mount.tsx", render(UI_MOUNT, request)),
        file("ui/src/remoteEntry.ts", render(UI_REMOTE, request)),
        file("ui/tsconfig.json", UI_TSCONFIG.into()),
        file("ui/vite.config.ts", render(UI_VITE, request)),
    ]
}

fn file(path: &str, body: String) -> RenderedFile {
    RenderedFile {
        path: PathBuf::from(path),
        body,
    }
}

fn render(template: &str, request: &ScaffoldRequest) -> String {
    template
        .replace("{id}", &request.id)
        .replace("{crate_id}", &request.id.replace('-', "_"))
        .replace("{tool}", &tool_name(request))
        .replace("{world}", WORLD)
}

fn render_manifest(request: &ScaffoldRequest) -> String {
    let caps = feature_caps(&request.features)
        .into_iter()
        .map(|cap| format!("    \"{cap}\","))
        .collect::<Vec<_>>()
        .join("\n");
    let ui = if request.features.contains(&Feature::Ui) {
        format!(
            r#"
[ui]
entry = "remoteEntry.js"
label = "{id}"
icon = "box"
scope = ["{id}.ping"]
"#,
            id = request.id
        )
    } else {
        String::new()
    };
    let native = if request.tier == Tier::Native {
        format!(
            r#"
[native]
exec = "{id}"
args = []
restart = "on-crash"
"#,
            id = request.id
        )
    } else {
        String::new()
    };
    render(MANIFEST, request)
        .replace("{tier}", request.tier.as_str())
        .replace("{caps}", &caps)
        .replace("{ui}", &ui)
        .replace("{native}", &native)
}

fn tool_name(request: &ScaffoldRequest) -> String {
    format!("{}.ping", request.id.replace('-', "."))
}

/// Reserved prefixes that a scaffolded extension id must not reuse. `devkit-build` is the build job-id
/// namespace (see `host::devkit::build`: `format!("devkit-build-{ts}")`); allowing it as a scaffold id
/// produced stray `devkit-build-<tier>-<ts>` folders indistinguishable from job records.
const RESERVED_ID_PREFIXES: &[&str] = &["devkit-build"];

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || id.starts_with('-')
        || id.ends_with('-')
    {
        bail!("extension id must be kebab-case ascii");
    }
    if let Some(prefix) = RESERVED_ID_PREFIXES
        .iter()
        .find(|p| id == **p || id.starts_with(&format!("{}-", p)))
    {
        bail!("extension id must not reuse reserved prefix '{prefix}'");
    }
    Ok(())
}

const CARGO_WASM: &str = include_str!("../templates/wasm/Cargo.toml.tmpl");
const CARGO_NATIVE: &str = include_str!("../templates/native/Cargo.toml.tmpl");
const MANIFEST: &str = include_str!("../templates/common/extension.toml.tmpl");
const BUILD_SH: &str = include_str!("../templates/common/build.sh.tmpl");
const WASM_LIB: &str = include_str!("../templates/wasm/src_lib.rs.tmpl");
const NATIVE_MAIN: &str = include_str!("../templates/native/src_main.rs.tmpl");
const UI_PACKAGE: &str = include_str!("../templates/ui/package.json.tmpl");
const UI_INDEX: &str = include_str!("../templates/ui/index.html.tmpl");
const UI_APP: &str = include_str!("../templates/ui/src_App.tsx.tmpl");
const UI_MOUNT: &str = include_str!("../templates/ui/src_mount.tsx.tmpl");
const UI_REMOTE: &str = include_str!("../templates/ui/src_remoteEntry.ts.tmpl");
const UI_TSCONFIG: &str = include_str!("../templates/ui/tsconfig.json.tmpl");
const UI_VITE: &str = include_str!("../templates/ui/vite.config.ts.tmpl");
