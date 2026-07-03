//! The boot seeder for the built-in agent definitions (agent-catalog scope) — the ONLY writer of the
//! reserved [`AGENT_DEFS_NS`] namespace, mirroring `seed_core_skills`. It parses the embedded
//! `agents.toml` manifest (overridable by a node-config file path via `LB_AGENT_CATALOG_TOML`), stamps
//! each entry's id with the `builtin.` prefix, and UPSERTs it into `_lb_agents`.
//!
//! **Idempotent + symmetric.** A re-boot re-UPSERTs the same records (LWW on the slug) — a no-op in
//! effect, and a node upgrade seeds new/changed entries cleanly. A node built WITHOUT the
//! `external-agent` feature still seeds the `open-interpreter.*` entries: the seed is symmetric;
//! whether they *appear* in the catalog is decided at list time by the node's runtime registry (never
//! an `if cloud` here).
//!
//! **Names only.** The manifest carries provider/model/env-NAME/base-url — never a key value.

use lb_store::{Store, StoreError};
use serde::Deserialize;

use super::model::{AgentDefinition, BUILTIN_PREFIX};
use super::store::{upsert_definition, AGENT_DEFS_NS};

/// The manifest embedded at build time. An operator can override it wholesale with a file path in
/// `LB_AGENT_CATALOG_TOML` (the core-skills embed precedent + one override path, no rebuild needed).
const EMBEDDED_MANIFEST: &str = include_str!("agents.toml");

/// The env var naming an override manifest path. When set and readable, its contents replace the
/// embedded manifest entirely (the operator owns the full built-in set).
const OVERRIDE_ENV: &str = "LB_AGENT_CATALOG_TOML";

/// One `[[agent]]` entry in the manifest. `id` here is the bare slug — the seeder prefixes `builtin.`.
#[derive(Debug, Deserialize)]
struct ManifestEntry {
    id: String,
    label: String,
    #[serde(default)]
    description: Option<String>,
    runtime: String,
    model_endpoint: super::model::DefinitionEndpoint,
}

/// The manifest document: a list of `[[agent]]` entries.
#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    agent: Vec<ManifestEntry>,
}

/// Load the manifest text: the `LB_AGENT_CATALOG_TOML` file if set + readable, else the embedded one.
fn manifest_text() -> String {
    if let Ok(path) = std::env::var(OVERRIDE_ENV) {
        if !path.is_empty() {
            match std::fs::read_to_string(&path) {
                Ok(text) => return text,
                Err(e) => {
                    tracing::warn!(
                        "boot: {OVERRIDE_ENV}={path:?} unreadable ({e}); using embedded agent catalog"
                    );
                }
            }
        }
    }
    EMBEDDED_MANIFEST.to_string()
}

/// Parse the manifest into the built-in definitions (ids prefixed `builtin.`). A malformed manifest is
/// a boot error, surfaced to the caller — a bad seed should fail loudly, not silently ship an empty
/// catalog.
pub fn builtin_definitions() -> Result<Vec<AgentDefinition>, StoreError> {
    let text = manifest_text();
    let manifest: Manifest = toml::from_str(&text)
        .map_err(|e| StoreError::Decode(format!("agent catalog manifest: {e}")))?;
    Ok(manifest
        .agent
        .into_iter()
        .map(|e| AgentDefinition {
            id: format!("{BUILTIN_PREFIX}{}", e.id),
            label: e.label,
            description: e.description,
            runtime: e.runtime,
            model_endpoint: e.model_endpoint,
            builtin: true,
        })
        .collect())
}

/// Seed the built-in agent definitions into the reserved [`AGENT_DEFS_NS`] namespace. Idempotent
/// (LWW UPSERT). Returns the seeded ids for the boot log. The ONLY writer of `_lb_agents`.
pub async fn seed_agent_definitions(store: &Store) -> Result<Vec<String>, StoreError> {
    let mut seeded = Vec::new();
    for def in builtin_definitions()? {
        upsert_definition(store, AGENT_DEFS_NS, &def).await?;
        seeded.push(def.id);
    }
    Ok(seeded)
}
