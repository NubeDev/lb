//! The one telemetry event schema (observability scope) — the shape every node emits AND the shape
//! the capped sink stores AND the shape the console reads. One vocabulary, three consumers. The
//! stored record is a flat object of top-level fields (queryable, indexable — the scope names
//! `ws`/`trace_id`/`level`/`source`/insert-seq indexes), NOT a nested `data` envelope, because the
//! console filters on these directly.

use serde::{Deserialize, Serialize};

/// The `telemetry` SurrealDB table name (capped by [`lb_store::capped_insert`]).
pub const TABLE: &str = "telemetry";

/// A log level — the bounded set the console filters on (no free-form severity strings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
            Level::Debug => "debug",
            Level::Trace => "trace",
        }
    }

    /// Parse a level from its string form; `None` for an unknown string.
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "error" => Level::Error,
            "warn" => Level::Warn,
            "info" => Level::Info,
            "debug" => Level::Debug,
            "trace" => Level::Trace,
            _ => return None,
        })
    }
}

/// The capability-decision outcome the console filters on (the security-relevant dimension: an
/// attempted access is itself the signal — audit scope's "deny half", mirrored here as a sample).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// The cap check allowed the call.
    Allow,
    /// The cap check denied the call (the security-interesting event).
    Deny,
    /// The call was allowed but the tool errored.
    Error,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::Allow => "allow",
            Outcome::Deny => "deny",
            Outcome::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "allow" => Outcome::Allow,
            "deny" => Outcome::Deny,
            "error" => Outcome::Error,
            _ => return None,
        })
    }
}

/// The stored telemetry record. Serialized to a JSON object of top-level fields and written via
/// [`lb_store::capped_insert`]; read back by `telemetry.query` filtered by these fields. `ws` is
/// mandatory — it is the field the read surface walls on (a ws-B query filters `ws = 'B'`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    pub level: String,
    pub ws: String,
    pub actor: String,
    pub tool: String,
    pub source: String,
    pub trace_id: String,
    pub outcome: String,
    pub ts: u64,
    pub msg: String,
    /// The already-redacted params digest (SHA-256:shape) — never the raw params.
    pub params_digest: String,
    /// Any extra structured fields the emitter attached (already redacted by the caller).
    #[serde(default)]
    pub fields: serde_json::Value,
}

impl TelemetryRecord {
    /// Build the stored JSON value (the body `capped_insert` stores; it injects `cap_key` + `seq`).
    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}
