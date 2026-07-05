//! The **persona** record shape (persona-model scope, sub-scope #1 of agent-personas). A persona is a
//! `{ granted_tools, grounding_skills, identity, extends }` bundle of already-shipped, grant-gated data
//! selected per workspace and applied at run assembly — a *focus*, not an entitlement. It **narrows**
//! the run (the advertised tool menu + the pinned skill set + an identity prompt); it never widens the
//! capability wall (`persona ∩ agent ∩ caller`, every dispatch re-checked).
//!
//! Two tiers, ONE shape — the `agent_definition` pattern (fourth reuse):
//!   - **built-in** — seeded from the embedded `personas.toml` into the reserved `_lb_personas`
//!     namespace, read-only to users (a `builtin.` id rejects create/update/delete before the caps
//!     gate). `builtin: true`.
//!   - **custom** — a workspace-authored record in the workspace namespace, full admin CRUD.
//!     `builtin: false`.
//!
//! **Names only, opaque ids (rule 10).** Every string in `granted_tools` is a raw MCP tool id or a
//! trailing-`*` glob; every string in `grounding_skills` is a skill id; `extends` holds persona ids.
//! Host code never branches on any of them — they resolve to records/catalog entries as opaque data,
//! exactly like the outbox `Target` treats an effect string.

use serde::{Deserialize, Serialize};

/// The reserved `builtin.` id prefix. A built-in persona's id always starts with it; a custom `create`
/// that names one is rejected (`BadInput`) before the caps gate, so the two tiers never collide — the
/// same rule as the agent-definition catalog and the core-skills `core.` prefix.
pub const BUILTIN_PREFIX: &str = "builtin.";

/// Is `id` a reserved built-in id? Read-only to users regardless of caps (checked before the caps
/// gate, like `is_builtin` for definitions).
pub fn is_builtin(id: &str) -> bool {
    id.starts_with(BUILTIN_PREFIX)
}

/// A discriminator column stamped on every stored persona so the generic `list` verb can select them
/// by an equality filter (the store's `list` filters on one `data.<field>`), independent of tier or
/// namespace. Never client-supplied.
pub const PERSONA_KIND: &str = "persona";

/// One persona — a curated focus a workspace picks for the agent. All list fields default empty so an
/// older/partial record still deserializes; `builtin` is stamped by the store on read, never trusted
/// from client input on a write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Persona {
    /// The stable id (slug). A built-in is `builtin.<slug>`; a custom id must NOT use that prefix.
    pub id: String,
    /// Human label for the picker (e.g. "Data analyst").
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The persona identity prompt — prepended to the in-house `SYSTEM_PROMPT` and folded at the head
    /// of the external runtime's goal. Short; the persona's *voice*, one source, both runtimes.
    #[serde(default)]
    pub identity: String,
    /// Tool ids (or trailing-`*` globs, e.g. `flows.*`) the persona advertises — OPAQUE data (rule 10).
    /// A **narrowing hint** over the run's reachable menu, never a grant: a listed tool the caller
    /// lacks is still denied at the wall. Unset (empty) means "no narrowing"; an explicit `[]` means a
    /// tool-less conversational persona (both allowed — see the scope's open-question 4).
    #[serde(default)]
    pub granted_tools: Vec<String>,
    /// Skill ids pinned at session start (grant-gated, fail-closed): their bodies are injected and the
    /// advertised catalog is filtered to this set. A pinned-but-ungranted skill fails the run at start.
    #[serde(default)]
    pub grounding_skills: Vec<String>,
    /// Persona ids whose tool/skill lists union into this one at read time (identity: child wins).
    /// Cycle-checked and depth-capped at write time (see `validate.rs`).
    #[serde(default)]
    pub extends: Vec<String>,
    /// OPTIONAL supervision floor (persona-coding sub-scope #4): a per-tool Allow/Ask/Deny preset
    /// applied via the shipped `agent.policy.set` machinery on activation. A floor, not a suggestion —
    /// tightening is free; loosening below it is an explicit admin write. `None` = no preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_preset: Option<PolicyPreset>,
    /// OPTIONAL runtime restriction (persona-coding sub-scope #4): the runtime ids this persona may run
    /// under. When set and non-empty, applying the persona with any other runtime fails at run start
    /// with a named error (the extension-builder is in-house-only until the external sandbox ships).
    /// `None`/empty = no restriction. Opaque runtime ids (rule 10).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtimes: Option<Vec<String>>,
    /// True for a seeded reserved-namespace built-in (read-only); false for a workspace custom entry.
    /// Set by the store on read — a write never trusts this from client input.
    #[serde(default)]
    pub builtin: bool,
}

/// A per-tool supervision preset (persona-coding #4). Each list holds tool ids (or trailing-`*` globs)
/// mapped to a policy effect; `apply.rs` translates it into `agent.policy.set` rules on activation.
/// Names only, opaque ids (rule 10).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyPreset {
    /// Tools that require a human decision before dispatch (durable suspension).
    #[serde(default)]
    pub ask: Vec<String>,
    /// Tools denied outright (fed back as a policy denial, never dispatched).
    #[serde(default)]
    pub deny: Vec<String>,
}
