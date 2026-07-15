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

pub mod catalog;
pub mod control;
mod engine;
pub mod grid;
mod meter;
mod runtime;
mod sandbox;
pub mod seam;
mod verbs;

pub use catalog::{FnEntry, CATALOG};
pub use control::{ControlIntent, RunControl};

// The polars-backed `Frame` surface (data-stdlib-scope). Linked only behind the `frames` cargo
// feature (default on); Phase 0 wires the dependency so the link resolves + the artifact-size delta
// is measurable. The rhai `register` entry point + the Frame verb surface land in Phase 2; until
// then this re-export is the single seam a future `verbs/frame.rs` reaches through.
#[cfg(feature = "frames")]
pub use lb_frame as frame;

pub use engine::{AiLimits, JobBinding, RuleEngine, RunOptions};
pub use grid::{dynamic_to_json, json_to_dynamic};
pub use meter::{AiMeter, WriteMeter};
pub use runtime::{
    AiBudget, Finding, GridJson, LogLine, ParamKind, Rule, RuleError, RuleOutput, RuleParam,
    RuleRun,
};
pub use sandbox::RuleLimits;
pub use seam::{
    AiCompletion, AiSeam, DataSeam, JobSeam, MessagingSeam, SchemaColumn, SeamError, SourceKind,
};
