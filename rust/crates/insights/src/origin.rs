//! `Origin` — the producer provenance of an insight (insights umbrella scope).
//!
//! Records *what raised it* and *from which run*. The `ref` field is an opaque string (a rule id,
//! a flow id, an agent def, an ext tool id) — the core **never branches on it** (rule 10): a rule
//! raises with `kind: Rule, ref: <rule-id>`; a flow with `kind: Flow, ref: <flow-id>`; the kind is
//! data the host stamps from the producer door it came in through, never caller-supplied (the
//! caller cannot lie about which door it used). `run` carries the originating `flow_run`/`job` id
//! when applicable ("where it was triggered from") — the deep link on the Insights detail drawer.

use serde::{Deserialize, Serialize};

/// The kind of producer that raised an insight. Set by the host from the producer door the raise
/// came in through (the rhai handle → `Rule`, the flow sink node → `Flow`, the plain MCP verb for
/// an agent/ext/manual). Domain-free (rule 10): no provider, no vertical name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OriginKind {
    Rule,
    Flow,
    Agent,
    Ext,
    Manual,
}

/// The provenance of an insight. `ref` is an opaque id; `run` is the optional flow-run/job id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Origin {
    /// What kind of producer raised this (rule / flow / agent / ext / manual).
    pub kind: OriginKind,
    /// The producer's stable id — a rule id, flow id, agent def id, or ext tool id. Opaque to
    /// the host; the UI deep-links it.
    #[serde(rename = "ref")]
    pub reference: String,
    /// The originating run/job id when applicable (`flow_run:…`, `job:…`). Absent for a manual
    /// or direct MCP raise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

impl Origin {
    /// Build an origin. Kept explicit (no `Default`) — an origin without a `ref` is a bug.
    pub fn new(kind: OriginKind, reference: impl Into<String>, run: Option<String>) -> Self {
        Self {
            kind,
            reference: reference.into(),
            run,
        }
    }
}
