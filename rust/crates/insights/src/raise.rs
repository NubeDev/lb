//! `raise` — the producer's WRITE verb (insights umbrella scope + occurrences scope).
//!
//! Idempotent on `(ws, dedup_key)`. The dedup/re-open decision branch — open/acked ⇒ bump
//! `count`+`last_ts` (status untouched, an acked fault re-firing doesn't re-page); resolved ⇒
//! **re-open** (status back to `open`, count continues); no matching key ⇒ create — is the
//! load-bearing logic of this verb.
//!
//! Every raise also appends one occurrence row (occurrences scope) — an empty `occurrence` is
//! still the firing log. `producer` is host-stamped from the raising principal (un-spoofable).
//! After the write, the host fires the raise-time matcher (subscriptions scope) and the
//! `insight.watch` bus event (umbrella scope) — those are the HOST layer's job, not this verb.
//!
//! **STUB**: the dedup decision + occurrence append + matcher triggering are deferred to the
//! implementing session — see the scaffold-session punch-list. The signature + types are stable;
//! the body is a `todo!()` so a green-but-lying stub is impossible.

use lb_store::Store;

use crate::error::InsightsError;
use crate::insight::Insight;
use crate::origin::Origin;
use crate::severity::Severity;

/// The optional per-firing occurrence delta (occurrences scope). Whether or not this is present,
/// every raise appends one occurrence row — `data`/`severity` here just shape it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RaiseOccurrence {
    /// Opaque JSON delta — score, reading, txn ref. ≤ 2 KB serialized or the whole raise rejects.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub data: serde_json::Value,
    /// The severity THIS firing carried (defaults to the raise's top-level `severity`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<Severity>,
}

/// The caller-supplied raise input. `dedup_key`/`severity`/`title`/`origin` are required; `body`,
/// `tags`, `occurrence` are optional. `tags` rides the shipped tag graph (applied by the host
/// layer after the record write — this crate is tag-graph-agnostic).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RaiseInput {
    pub dedup_key: String,
    pub severity: Severity,
    pub title: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub body: serde_json::Value,
    pub origin: Origin,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub tags: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<RaiseOccurrence>,
    /// Caller-injected logical timestamp (no wall-clock — testing §3).
    pub ts: u64,
    /// Host-stamped from the raising principal (`user:…`/`key:…`/`ext:…`) — un-spoofable.
    pub producer: String,
}

/// The raise outcome — what the host returns to the producer / UI. `created` distinguishes a
/// brand-new insight from a count-bump on an existing one (the UI badge + the matcher's
/// first-key breakthrough both care).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RaiseOutcome {
    /// The insight's stable id.
    pub id: String,
    /// Post-raise status (always the prior status for open/acked; `open` for create + re-open).
    pub status: crate::status::Status,
    /// Post-raise lifetime count.
    pub count: u64,
    /// True if this raise created a brand-new insight (first time this `dedup_key` was seen).
    pub created: bool,
    /// True if this raise re-opened a previously-`resolved` insight.
    pub reopened: bool,
}

/// Raise an insight in workspace `ws`. Idempotent on `(ws, dedup_key)`. See [`RaiseInput`] for
/// the fields and the module doc for the dedup/re-open decision.
// SCOPE: docs/scope/insights/insights-scope.md §"Dedup / flap suppression" + §"MCP surface"
// SCOPE: docs/scope/insights/insight-occurrences-scope.md §"Verb surface"
pub async fn raise(
    _store: &Store,
    _ws: &str,
    _input: RaiseInput,
) -> Result<RaiseOutcome, InsightsError> {
    // 1. Look up the existing insight by dedup_key (`dedup_lookup`).
    // 2. Existing open/acked → bump count/last_ts + (severity takes newest) + append occurrence.
    //    Status UNTOUCHED (an acked fault re-firing doesn't re-page).
    // 3. Existing resolved → re-open (status=open, count continues), append occurrence, fire
    //    the matcher with IntentKind::Reopen.
    // 4. No match → mint ULID, create the row (count=1, first_ts=ts), append occurrence, fire
    //    the matcher with IntentKind::Raise (first-key breakthrough).
    // 5. Apply `tags` through the host's tag path (NOT here — this crate is tag-graph-agnostic;
    //    the host calls `tags_add` after the write).
    // 6. Return the outcome (the host then publishes the `insight.watch` bus event).
    todo!("insights: dedup/re-open decision + occurrence append — SCOPE: insights-scope.md §Dedup")
}

/// Read the parent insight by id (re-exported for the host service so it can read the post-raise
/// state without reaching into the record module).
pub async fn read_insight(
    _store: &Store,
    _ws: &str,
    _id: &str,
) -> Result<Option<Insight>, InsightsError> {
    todo!("insights: read insight by id — SCOPE: insights-scope.md §MCP surface (get)")
}
