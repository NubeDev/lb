use std::path::Path;

use anyhow::{anyhow, Context, Result};
use toml::Value;

use crate::artifacts::collect_artifacts;
use crate::toolchain::{ProcessToolchain, Toolchain};
use crate::{InspectReport, Tier, ToolchainReadiness};

pub fn inspect_extension(path: &Path) -> Result<InspectReport> {
    let manifest_path = path.join("extension.toml");
    let manifest = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("read {}", manifest_path.display()))?;
    let parsed: Value = toml::from_str(&manifest).context("parse extension.toml")?;
    let id = table_str(&parsed, "extension", "id")?;
    let tier = match table_str(&parsed, "runtime", "tier")?.as_str() {
        "wasm" => Tier::Wasm,
        "native" => Tier::Native,
        other => return Err(anyhow!("unknown runtime tier {other}")),
    };
    let tools = parsed
        .get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    let caps = parsed
        .get("capabilities")
        .and_then(|c| c.get("request"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|c| c.as_str().map(str::to_string))
        .collect();
    let artifacts = collect_artifacts(path, tier);
    Ok(InspectReport {
        id,
        tier,
        tools,
        caps,
        // "built" is now grounded in a real compiled binary/component on disk, not just the presence
        // of a `release/` dir (which `cargo` creates even for a failed or aborted build). A UI
        // artifact alone doesn't count — the extension isn't loadable without its binary.
        built: artifacts
            .iter()
            .any(|a| a.kind == "wasm" || a.kind == "native-bin"),
        toolchain: readiness(path),
        artifacts,
    })
}

fn table_str(parsed: &Value, table: &str, key: &str) -> Result<String> {
    parsed
        .get(table)
        .and_then(|t| t.get(key))
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("manifest missing [{table}] {key}"))
}

fn readiness(path: &Path) -> ToolchainReadiness {
    let toolchain = ProcessToolchain;
    ToolchainReadiness {
        cargo: toolchain.ready("cargo"),
        pnpm: !path.join("ui").is_dir() || toolchain.ready("pnpm"),
        wasm32_wasip2: toolchain.wasm_target_ready(),
    }
}
