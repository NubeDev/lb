//! The **insight** record — one row per *thing we know* (a card, an AHU, a meter) keyed by
//! `(ws, dedup_key)`, with severity, provenance, and an `open → acked → resolved` lifecycle
//! (insights umbrella scope).
//!
//! State, like every `lb_*` record: it lives in `lb_store` behind the workspace wall (§7). The
//! bus moves a copy as motion (`insight.watch`); the store keeps this as the durable record (§3.3).
//! `producer` is host-stamped from the raising principal (un-spoofable, the ingest pattern); all
//! timestamps are caller-injected logical timestamps (testing §3 — no wall-clock in core).
//!
//! `count`/`first_ts`/`last_ts` are the LIFETIME occurrence accounting (monotone) — the parent
//! truth, independent of the occurrence ring's eviction (occurrences scope). The ring is the
//! recent evidence window; these three are the forever count.

use serde::{Deserialize, Serialize};

use crate::evidence::Evidence;
use crate::origin::Origin;
use crate::severity::Severity;
use crate::status::Status;

/// The store table all insights live in. One table per workspace namespace; `dedup_key` +
/// `status` + `severity` are `data` fields (so the list view is a filtered scan, not a new table).
pub const OCC_TABLE: &str = "insight";

/// A durable insight record. Stable on `(ws, dedup_key)` — re-raising the same key bumps
/// `count`/`last_ts` (or re-opens if `resolved`), never a duplicate row.
///
/// Not `Eq`: `evidence.threshold` is an `f64` (a threshold is a real quantity in the series' own
/// units, so the float is the honest type). Compare with `PartialEq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Insight {
    /// Stable id (ULID), unique within the workspace. Host-assigned at first raise.
    pub id: String,
    /// Caller-supplied stable identity — `"rule:hunting:ahu-2"`, `"fraud:4421"`. The dedup key.
    /// High-cardinality identity (a card, an equip) lives HERE, never in tags (umbrella scope's
    /// tag-cardinality rule).
    pub dedup_key: String,
    /// The severity of the latest firing (an occurrence may carry its own; the parent holds newest).
    pub severity: Severity,
    /// One-line human title.
    pub title: String,
    /// Opaque JSON detail — evidence rows, scores, links. Free-form; producers own the shape.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub body: serde_json::Value,
    /// The data that proves this finding — datasource + the plottable series + the threshold and
    /// window judged (`insight-evidence-scope.md`). Optional: absent on every record written before
    /// the field landed and on every producer that states none, and a reader that ignores it is
    /// unaffected. **Refreshed on every raise that supplies one** — unlike `title`/`body`, which are
    /// first-raise-wins; see the note at the dedup arm in `raise.rs`.
    ///
    /// Echoed by `insight.get`; **omitted by `insight.list`** (it would bloat every page of a
    /// many-record roster for a field only the detail view uses, and the SQL it carries is schema
    /// disclosure the narrower read already implies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
    /// Producer provenance — what raised it, from which run.
    pub origin: Origin,
    /// The lifecycle status.
    pub status: Status,
    /// Who moved the status last (a `user:…` subject). Absent while `open` and un-acked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_by: Option<String>,
    /// Logical timestamp of the last status transition (no wall-clock — testing §3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_ts: Option<u64>,
    /// Lifetime raise count (monotone — may exceed the occurrence ring's stored rows).
    pub count: u64,
    /// Logical timestamp of the first raise (monotone per insight).
    pub first_ts: u64,
    /// Logical timestamp of the most recent raise (advances on every raise).
    pub last_ts: u64,
    /// Host-stamped raising principal (`user:…`/`key:…`/`ext:…`) — un-spoofable (ingest pattern).
    pub producer: String,
}
