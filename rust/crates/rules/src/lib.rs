//! `lb-rules` — the embedded, sandboxed rules/processing engine (rules-engine-scope.md).
//!
//! A workspace authors a small Rhai script that reads data (through host-mediated, capability-checked
//! seams), transforms it (a lazy, column-oriented `Grid` + timeseries plan-builders), calls AI (the
//! gateway, metered + fenced), and emits findings/alerts. The crate links ONLY rhai (+ serde): no
//! DataFusion, no store, no socket. A rule reaches the platform only through the [`seam`] traits the
//! host implements against the real `store.query`/`series.*`/`federation.query`/AI-gateway/inbox-outbox.
//!
//! ## Attribution
//!
//! Ported from the `rubix-cube` rules engine (`rust/rubix-cube/rbx-server/src/rules/`), MIT/Apache-2.0,
//! same repo lineage. **Lifted verbatim:** the rhai sandbox + governors ([`sandbox`]), the lazy `Grid`/
//! `Col`/`GroupedGrid` plan model ([`grid`]), the timeseries plan-builders ([`verbs`]), the `AiMeter`
//! budget ([`meter`]), the nsql re-validation fence (in [`verbs`]). **Re-seamed:** grid `collect`
//! calls the host data seam (`store.query`/`series.*` or
//! `federation.query`) instead of a local DataFusion engine; `ai.*` re-points at the AI-gateway;
//! `alert` routes to inbox/outbox (host-side). **Re-keyed:** `project_id` → `workspace`.

mod engine;
pub mod grid;
mod meter;
mod runtime;
mod sandbox;
pub mod seam;
mod verbs;

pub use engine::{AiLimits, RuleEngine};
pub use grid::{dynamic_to_json, json_to_dynamic};
pub use meter::AiMeter;
pub use runtime::{
    AiBudget, Finding, GridJson, LogLine, Rule, RuleError, RuleOutput, RuleParam, RuleRun,
};
pub use sandbox::RuleLimits;
pub use seam::{AiCompletion, AiSeam, DataSeam, SchemaColumn, SourceKind};
