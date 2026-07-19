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

use lb_store::{new_ulid, write, Store};

use crate::error::InsightsError;
use crate::evidence::{validate_evidence_size, Evidence};
use crate::insight::{Insight, OCC_TABLE};
use crate::insight_id::{dedup_lookup, record_id};
use crate::intent::IntentKind;
use crate::occ_append::{append_occurrence, validate_occurrence_size};
use crate::occurrence::Occurrence;
use crate::origin::Origin;
use crate::severity::Severity;
use crate::status::Status;

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
/// `tags`, `occurrence`, `evidence` are optional. `tags` rides the shipped tag graph (applied by
/// the host layer after the record write — this crate is tag-graph-agnostic).
///
/// Not `Eq` for the same reason [`crate::Insight`] isn't — `evidence.threshold` is an `f64`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RaiseInput {
    pub dedup_key: String,
    pub severity: Severity,
    pub title: String,
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub body: serde_json::Value,
    /// The data that proves the finding (`insight-evidence-scope.md`). Optional; when present it
    /// **overwrites** any stored evidence, and when absent the stored value is left alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<Evidence>,
    pub origin: Origin,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub tags: std::collections::BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<RaiseOccurrence>,
    /// Caller-injected logical timestamp (no wall-clock — testing §3). Serde-defaults to `0` so a
    /// producer door may omit it; the host layer (`insight_raise`) backfills the wall-clock on `0`
    /// (this crate stays wall-clock-free). A deterministic caller passes an explicit non-zero `ts`.
    #[serde(default)]
    pub ts: u64,
    /// Host-stamped from the raising principal (`user:…`/`key:…`/`ext:…`) — un-spoofable. Serde
    /// defaults to empty so the MCP door can deserialize a caller's body that (correctly) omits it;
    /// the host layer overwrites it from the principal before the write (a caller value is ignored).
    #[serde(default)]
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
    /// The dedup key (echoed so the host can build the matcher's `InsightView` without a re-read).
    pub dedup_key: String,
    /// This firing's severity (the matcher's floor check + the digest's `max_severity` rollup).
    pub severity: crate::severity::Severity,
    /// The intent kind the raise-time matcher should carry — `Reopen` on a re-open, `Escalate`
    /// when this firing's severity is strictly higher than the prior, else `Raise`. Drives the
    /// ladder's breakthrough rules (notify scope). Host-facing only; the UI ignores it.
    pub kind: IntentKind,
}

/// Raise an insight in workspace `ws`. Idempotent on `(ws, dedup_key)`. See [`RaiseInput`] for
/// the fields and the module doc for the dedup/re-open decision.
// SCOPE: docs/scope/insights/insights-scope.md §"Dedup / flap suppression" + §"MCP surface"
// SCOPE: docs/scope/insights/insight-occurrences-scope.md §"Verb surface"
pub async fn raise(
    store: &Store,
    ws: &str,
    input: RaiseInput,
    ring_cap: usize,
) -> Result<RaiseOutcome, InsightsError> {
    // Validate the occurrence size UP FRONT — an oversize payload rejects the whole raise and
    // leaves no parent row (occurrences scope: never a partial write, never silent truncation).
    let occ = input.occurrence.clone().unwrap_or(RaiseOccurrence {
        data: serde_json::Value::Null,
        severity: None,
    });
    let occ_severity = occ.severity.unwrap_or(input.severity);
    let firing = Occurrence {
        seq: 0, // set below once we know the parent's post-bump count
        ts: input.ts,
        severity: occ_severity,
        data: occ.data.clone(),
    };
    validate_occurrence_size(&firing)?;
    // Same contract for the evidence descriptor — reject before any write, never a partial raise.
    if let Some(ev) = &input.evidence {
        validate_evidence_size(ev)?;
    }

    let existing = dedup_lookup(store, ws, &input.dedup_key).await?;
    let (insight, created, kind) = match existing {
        Some(mut prior) => {
            let prev_severity = prior.severity;
            // Bump the lifetime accounting on every raise.
            prior.count += 1;
            prior.last_ts = input.ts;
            prior.severity = input.severity;
            // Evidence REFRESHES on re-raise — deliberately unlike `title`/`body`/`origin`, which
            // stay first-raise-wins just below. Evidence is a *binding*, not a historical fact: a
            // rule edited to query a renamed table would otherwise leave every existing insight
            // bound to a query that no longer runs, permanently, with no way to heal short of
            // deleting the record. A raise that omits evidence leaves the stored value alone, so a
            // producer can stop sending it without blanking the binding.
            // SCOPE: docs/scope/insights/insight-evidence-scope.md §"How it fits" (Dedup)
            if input.evidence.is_some() {
                prior.evidence = input.evidence.clone();
            }
            let reopened = prior.status == Status::Resolved;
            if reopened {
                // A resolved insight firing again re-opens (count continues). Status → open; the
                // prior resolver/ts are cleared (a fresh open lifecycle).
                prior.status = Status::Open;
                prior.status_by = None;
                prior.status_ts = None;
            }
            // Escalation = strictly higher severity than the prior firing (drives a breakthrough).
            let kind = if reopened {
                IntentKind::Reopen
            } else if input.severity.rank() > prev_severity.rank() {
                IntentKind::Escalate
            } else {
                IntentKind::Raise
            };
            (prior, false, kind)
        }
        None => {
            // First time this dedup_key is seen — mint a fresh insight.
            let id = new_ulid();
            let insight = Insight {
                id,
                dedup_key: input.dedup_key.clone(),
                severity: input.severity,
                title: input.title.clone(),
                body: input.body.clone(),
                evidence: input.evidence.clone(),
                origin: input.origin.clone(),
                status: Status::Open,
                status_by: None,
                status_ts: None,
                count: 1,
                first_ts: input.ts,
                last_ts: input.ts,
                producer: input.producer.clone(),
            };
            (insight, true, IntentKind::Raise)
        }
    };

    // Persist the parent (upsert by id).
    let value = serde_json::to_value(&insight)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, OCC_TABLE, &record_id(&insight.id), &value).await?;

    // Append one occurrence row (seq = the parent's post-bump lifetime count — monotone per
    // insight). `ring_cap == 0` stores nothing but the parent count still moved (above).
    let firing = Occurrence {
        seq: insight.count,
        ..firing
    };
    append_occurrence(store, ws, &insight.id, &firing, ring_cap).await?;

    Ok(RaiseOutcome {
        id: insight.id,
        status: insight.status,
        count: insight.count,
        created,
        reopened: kind == IntentKind::Reopen,
        dedup_key: insight.dedup_key,
        severity: insight.severity,
        kind,
    })
}

/// Read the parent insight by id (re-exported for the host service so it can read the post-raise
/// state without reaching into the record module).
pub async fn read_insight(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<Insight>, InsightsError> {
    Ok(crate::get::get(store, ws, id).await?)
}
