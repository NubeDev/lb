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

use crate::analysis::Analysis;
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
    /// The producer's own REASONING about this finding — why it fired, the metric judged, the
    /// benchmark, the deviation, the estimated impact (`insight-analysis-scope.md`). The statement
    /// beside `evidence`'s data binding; a closed struct so every consumer renders the same labels.
    ///
    /// Optional and additive: absent on every record written before the field landed and on every
    /// producer that states none. **Refreshed on every raise that supplies one** (like `evidence`,
    /// and for a stronger reason: a deviation of "-100%" from firing #1 displayed beside
    /// `count: 47` is actively misleading — worse than absent). A raise that omits it leaves the
    /// stored value alone.
    ///
    /// Echoed by `insight.get`; **omitted by `insight.list`** — the same boundary `evidence` holds,
    /// for the same two reasons: six prose fields per row would bloat every page of a roster for
    /// data only the drawer uses, and that `get`-only boundary is what contains the free text
    /// producers will populate from anything in scope (site names, occupancy, tenant behaviour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Analysis>,
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
    /// Who OWNS this finding — the human triage plane's one axis
    /// (`insight-triage-scope.md`). A **subject, not a user id**: `user:priya` or
    /// `team:mechanical` are both legal, the same discipline [`Insight::status_by`] has, so
    /// queue-style ownership ("the mechanical crew owns this") works without a breaking read-side
    /// change later. `None` = unassigned (the triage queue's primary view).
    ///
    /// **Untouched by `raise`, forever.** This is neither producer-owned (like `title`/`body`,
    /// first-raise-wins) nor transition-owned (like `status_by`): it is a *human fact about the
    /// finding*, so there is no `assigned_to` on `RaiseInput` and no arm of the raise path may set,
    /// clear, or read it. A flapping sensor re-raising every 15 minutes must never silently
    /// un-assign the technician who took the job — **including on the re-open arm**, where
    /// `status_by`/`status_ts` DO clear: the fault came back and it is still Priya's.
    ///
    /// Echoed by **both** `insight.get` and `insight.list` (the owner column) — the tag-echo
    /// boundary, not the `evidence` one: the rule is "does a roster column need it", and this is the
    /// 6th column operators ask for. The comment thread stays `get`-only for the opposite reason.
    ///
    /// A subject outlives its membership: an insight assigned to a removed member keeps the stale
    /// subject (resolved decision 3) and a UI must render an unresolvable assignee as
    /// "unknown (removed)" rather than blank, so an orphaned queue is visible instead of empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<String>,
    /// Lifetime raise count (monotone — may exceed the occurrence ring's stored rows).
    pub count: u64,
    /// Logical timestamp of the first raise (monotone per insight).
    pub first_ts: u64,
    /// Logical timestamp of the most recent raise (advances on every raise).
    pub last_ts: u64,
    /// Host-stamped raising principal (`user:…`/`key:…`/`ext:…`) — un-spoofable (ingest pattern).
    pub producer: String,
    /// The insight's **tag facets, echoed** — a read-only projection of the tag graph
    /// (`insight-tag-echo-scope.md`). The dimension plane (building, asset type, priority, …) as
    /// flat `{k: v}`, so a roster renders dimension columns from `insight.list` alone instead of an
    /// N+1 `tags.find` per row.
    ///
    /// **The graph is the source of truth.** This is written only by the raise path, from
    /// `tags.of` on the insight entity (the union across ALL raises of the dedup key) — never from
    /// one raise's declared `tags`, and never by a caller (host-computed like `producer`).
    /// Refreshed on every raise, so an out-of-band `tags.*` change self-heals on the next firing.
    /// Filtering (`insight.list { tags }`) deliberately keeps reading the graph, not this echo — a
    /// filter over a projection returns wrong rows while the projection is behind.
    ///
    /// Echoed by **both** `insight.get` and `insight.list` — the deliberate divergence from
    /// [`Insight::evidence`], whose boundary rule ("does the roster render it") puts it on `get`
    /// only. Empty on every record written before the field landed; a reader ignoring it is
    /// unaffected.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub tags: std::collections::BTreeMap<String, String>,
}
