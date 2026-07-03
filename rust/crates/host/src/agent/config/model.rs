//! The per-workspace **agent config** record shape (agent-config scope). One record per workspace —
//! `workspace_agent_config:[ws]` — holding the workspace's chosen **default runtime id** and an
//! optional **model endpoint**. Every field is nullable so a patch (`agent.config.set`) MERGEs: a
//! present field sets that axis, an absent field leaves it untouched (same semantics as `Prefs`).
//!
//! **No secret values.** `api_key_env` is an env-var NAME (like `profiles.rs`'s [`ModelEndpoint`]),
//! never the key itself — the actual secret stays in the node env / `lb-secrets`. This mirrors the
//! external-agent umbrella's "the driver passes the *name*, never the value".

use serde::{Deserialize, Serialize};

/// A model endpoint the workspace's agent routes through — provider/model/env-name/base-url. All
/// nullable; a partial patch merges. Names only, never a secret value.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEndpointPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The env-var NAME holding the key — never the key value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// A **secret PATH** into `lb-secrets` holding the key (a name, never the value). Lets a workspace
    /// key its ACTIVE-pick model without cloning a built-in (the active `agent.config` is
    /// workspace-scoped and can own a sealed secret path). Resolution precedence at model-call time:
    /// `api_key_secret` (sealed) → `api_key_env` (node env) → unset — see
    /// [`resolve_endpoint_key`](crate::agent::resolve_endpoint_key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// The stored/patch shape of a workspace's agent config. `default_runtime` is a runtime id validated
/// against the node's registry on write; `model_endpoint` is a nested nullable object. An unset field
/// means "inherit / not chosen" — structurally, not via a sentinel.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentConfig {
    /// The workspace's chosen default runtime id (must exist in the node registry at write time).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_runtime: Option<String>,
    /// The model endpoint the agent routes through (names only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_endpoint: Option<ModelEndpointPatch>,
    /// The **active definition id** the workspace picked (`agent.def` catalog id). First-class so
    /// "which agent is active" is a stored fact, not re-derived from `default_runtime` (active-agent-
    /// wiring scope): the UI badge, rules, and the per-workspace model resolver read this ONE field.
    /// Additive + optional (back-compat: an old config with no `active_definition` still resolves via
    /// the `default_runtime` fallback). Written by the pick alongside the copied endpoint fields; LWW
    /// like every other field (an offline double-deliver UPSERTs idempotently).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_definition: Option<String>,
}
