//! The **agent definition** record shape (agent-catalog scope). A definition is a named preset =
//! `{ id, label, description?, runtime, model_endpoint }` — one of two tiers, ONE shape:
//!   - **built-in**  — seeded from the embedded `agents.toml` manifest into the reserved `_lb_agents`
//!     namespace, read-only to users (a `builtin.` id rejects create/update/delete). `builtin: true`.
//!   - **custom**    — a workspace-authored record in the workspace namespace, full admin CRUD.
//!     `builtin: false`.
//!
//! **No secret values.** `model_endpoint.api_key_env` is an env-var NAME (mirrors the shipped
//! [`ModelEndpointPatch`](crate::agent::ModelEndpointPatch) and `profiles.rs`' `ModelEndpoint`); the
//! actual key stays in the node env / `lb-secrets`, never in a definition record or the manifest.

use serde::{Deserialize, Serialize};

/// The reserved `builtin.` id prefix. A built-in definition's id always starts with it; a custom
/// `create` that names one is rejected (`Reserved`), so the two tiers can never collide — exactly the
/// core-skills `core.` prefix rule.
pub const BUILTIN_PREFIX: &str = "builtin.";

/// Is `id` a reserved built-in id? Read-only to users regardless of caps (checked before the caps
/// gate, like `is_core` for skills).
pub fn is_builtin(id: &str) -> bool {
    id.starts_with(BUILTIN_PREFIX)
}

/// A model endpoint a definition binds — provider/model/env-name/base-url. Names only, never a secret
/// value. Unlike the config patch, a definition carries a *concrete* endpoint (provider + model are
/// required for a preset to be meaningful); `api_key_env` / `base_url` stay optional.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionEndpoint {
    pub provider: String,
    pub model: String,
    /// The env-var NAME holding the key — never the key value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// A **secret PATH** (a name, e.g. `agent/zaicoding-key`) into `lb-secrets` holding the key —
    /// still never the key value (names-only, §6.7). Resolved at model-call time by
    /// [`resolve_endpoint_key`](crate::agent::resolve_endpoint_key): `api_key_secret` (sealed) →
    /// `api_key_env` (node env) → unset. The value lives ONLY in `lb-secrets`, written through the
    /// shipped sealed `secret.set` — it never lands in a definition record, manifest, or log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// One catalog entry: a named `(runtime, model_endpoint)` preset. `builtin` distinguishes the seeded
/// read-only tier from a workspace's custom definitions; it is set by the store layer on read (never
/// trusted from client input on a write).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// The stable id (slug). A built-in is `builtin.<slug>`; a custom id must NOT use that prefix.
    pub id: String,
    /// Human label for the picker (e.g. "In-house — Z.AI GLM-4.6").
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The runtime id this preset binds — validated against the node registry on a custom write.
    pub runtime: String,
    /// The model endpoint (names only) this preset routes through.
    pub model_endpoint: DefinitionEndpoint,
    /// True for a seeded reserved-namespace built-in (read-only); false for a workspace custom entry.
    /// Set by the store on read — a write never trusts this from client input.
    #[serde(default)]
    pub builtin: bool,
}

/// A discriminator column stamped on every stored definition so the generic `list` verb can select
/// them by an equality filter (the store's `list` filters on one `data.<field>`), independent of tier
/// or namespace. Never client-supplied.
pub const DEFINITION_KIND: &str = "agent_definition";
