//! The boot seeder for the built-in personas (persona-model scope) — the ONLY writer of the reserved
//! [`PERSONA_NS`] namespace, mirroring `seed_agent_definitions` / `seed_core_skills`. It parses the
//! embedded `personas.toml` manifest (overridable by a node-config file path via `LB_PERSONA_CATALOG_TOML`),
//! stamps each entry's id with the `builtin.` prefix, and UPSERTs it into `_lb_personas`.
//!
//! **Idempotent + symmetric.** A re-boot re-UPSERTs the same records (LWW on the slug) — a no-op in
//! effect; a node upgrade seeds new/changed entries cleanly. No role branch: every node seeds the same
//! catalog (whether a persona's tools are *reachable* is decided at run assembly by the wall, never here).
//!
//! **Names only, opaque ids (rule 10).** The manifest carries tool ids, skill ids, and persona ids as
//! plain strings — the seeder never interprets them.
//!
//! The built-in persona *contents* (the exact tool/skill lists) are owned by `persona-catalog` (#3) and
//! `persona-coding` (#4); this file owns the *mechanism*. The embedded manifest ships whatever those
//! sub-scopes author into `personas.toml`.

use lb_store::{Store, StoreError};
use serde::Deserialize;

use super::model::{Persona, PolicyPreset, BUILTIN_PREFIX};
use super::store::{upsert_persona, PERSONA_NS};

/// The manifest embedded at build time. An operator can override it wholesale with a file path in
/// `LB_PERSONA_CATALOG_TOML` (the agent-catalog embed precedent + one override path, no rebuild needed).
const EMBEDDED_MANIFEST: &str = include_str!("personas.toml");

/// The env var naming an override manifest path. When set and readable, its contents replace the
/// embedded manifest entirely (the operator owns the full built-in set).
const OVERRIDE_ENV: &str = "LB_PERSONA_CATALOG_TOML";

/// One `[[persona]]` entry in the manifest. `id` here is the bare slug — the seeder prefixes `builtin.`.
/// (`extends` parents are authored WITH their full `builtin.` id, since a built-in may extend another
/// built-in by its resolved id.)
#[derive(Debug, Deserialize)]
struct ManifestEntry {
    id: String,
    label: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    identity: String,
    #[serde(default)]
    granted_tools: Vec<String>,
    #[serde(default)]
    grounding_skills: Vec<String>,
    #[serde(default)]
    extends: Vec<String>,
    #[serde(default)]
    surfaces: Vec<String>,
    #[serde(default)]
    policy_preset: Option<PolicyPreset>,
    #[serde(default)]
    runtimes: Option<Vec<String>>,
}

/// The manifest document: a list of `[[persona]]` entries.
#[derive(Debug, Deserialize, Default)]
struct Manifest {
    #[serde(default)]
    persona: Vec<ManifestEntry>,
}

/// Load the manifest text: the `LB_PERSONA_CATALOG_TOML` file if set + readable, else the embedded one.
fn manifest_text() -> String {
    if let Ok(path) = std::env::var(OVERRIDE_ENV) {
        if !path.is_empty() {
            match std::fs::read_to_string(&path) {
                Ok(text) => return text,
                Err(e) => {
                    tracing::warn!(
                        "boot: {OVERRIDE_ENV}={path:?} unreadable ({e}); using embedded persona catalog"
                    );
                }
            }
        }
    }
    EMBEDDED_MANIFEST.to_string()
}

/// Parse the manifest into the built-in personas (ids prefixed `builtin.`). A malformed manifest is a
/// boot error, surfaced to the caller — a bad seed should fail loudly, not silently ship an empty
/// catalog.
pub fn builtin_personas() -> Result<Vec<Persona>, StoreError> {
    let text = manifest_text();
    let manifest: Manifest = toml::from_str(&text)
        .map_err(|e| StoreError::Decode(format!("persona catalog manifest: {e}")))?;
    Ok(manifest
        .persona
        .into_iter()
        .map(|e| Persona {
            id: format!("{BUILTIN_PREFIX}{}", e.id),
            label: e.label,
            description: e.description,
            identity: e.identity,
            granted_tools: e.granted_tools,
            grounding_skills: e.grounding_skills,
            extends: e.extends,
            surfaces: e.surfaces,
            policy_preset: e.policy_preset,
            runtimes: e.runtimes,
            builtin: true,
        })
        .collect())
}

/// Seed the built-in personas into the reserved [`PERSONA_NS`] namespace. Idempotent (LWW UPSERT).
/// Returns the seeded ids for the boot log. The ONLY writer of `_lb_personas`.
pub async fn seed_personas(store: &Store) -> Result<Vec<String>, StoreError> {
    let mut seeded = Vec::new();
    for persona in builtin_personas()? {
        upsert_persona(store, PERSONA_NS, &persona).await?;
        seeded.push(persona.id);
    }
    Ok(seeded)
}
