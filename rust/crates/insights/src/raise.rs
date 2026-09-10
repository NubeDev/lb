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

use crate::analysis::{validate_analysis, Analysis};
use crate::caveat::caveats_for;
use crate::error::InsightsError;
use crate::evidence::{validate_evidence_size, Evidence};
use crate::insight::{Insight, OCC_TABLE};
use crate::insight_id::{dedup_lookup, record_id};
use crate::intent::IntentKind;
use crate::occ_append::{append_occurrence, validate_occurrence_size};
use crate::occurrence::Occurrence;
use crate::origin::Origin;
use crate::pattern::{bump_month, derive_pattern, empty_month_hist, Pattern};
use crate::severity::Severity;
use crate::status::Status;
use crate::vocab::{check_value, read_vocab, TagVocab, CATEGORY_KEY};

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
    /// The producer's reasoning about the finding (`insight-analysis-scope.md`). Optional; when
    /// present it **overwrites** any stored analysis, and when absent the stored value is left
    /// alone. Refreshes independently of `evidence` — omission means "unchanged" for each, and a
    /// producer that changes its query but not its prose is a producer bug, not a reason to couple
    /// two fields whose lifetimes are unrelated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<Analysis>,
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
    /// True if this firing landed with a non-empty [`crate::Insight::caveats`] — an open
    /// data-quality finding on the same subjects undermines it (`caveat.rs`).
    ///
    /// Echoed on the outcome, not left for the host to re-read, because the host's very next step
    /// is to build the matcher's `InsightView` and **a caveated finding must never break through**
    /// (`ladder.rs`). Serde-defaults to `false` so an outcome decoded from an older node is
    /// un-caveated rather than undeliverable.
    #[serde(default)]
    pub caveated: bool,
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
    // And for the analysis object — shape (a `value` needs a `unit`; an all-absent `Quantity` is
    // refused) plus the whole-object cap, all before any write.
    if let Some(an) = &input.analysis {
        validate_analysis(an)?;
    }

    // The workspace's own declared `category` vocabulary — read ONCE, used twice: to validate a
    // declared category (below) and to learn which values GATE other findings (the caveat stamp
    // further down). `None` ⇒ the workspace declared none, and both uses are no-ops. **lb ships no
    // default value list** (rule 10 — `vocab.rs`), so an unseeded workspace behaves exactly as it
    // did before this existed.
    // SCOPE: docs/scope/insights/case-plane-scope.md §"Vocabulary"
    let vocab: Option<TagVocab> = read_vocab(store, ws, CATEGORY_KEY).await?;
    if let Some(declared) = input.tags.get(CATEGORY_KEY) {
        // Validated BEFORE any write, like the three size guards above: a bad category rejects the
        // whole raise rather than landing a record with a value nothing can group by.
        check_value(vocab.as_ref(), declared)?;
    }

    let existing = dedup_lookup(store, ws, &input.dedup_key).await?;
    let (mut insight, created, kind) = match existing {
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
            // `analysis` refreshes on the same on-supply rule, for a stronger reason than evidence
            // had: these fields exist to describe the CURRENT state of the finding, so a deviation
            // of "-100%" computed at firing #1 sitting beside `count: 47` is worse than absent. An
            // omitting raise leaves the stored value alone — the two fields refresh independently.
            // SCOPE: docs/scope/insights/insight-analysis-scope.md §"Resolved decisions" (1, 4)
            if input.analysis.is_some() {
                prior.analysis = input.analysis.clone();
            }
            let reopened = prior.status == Status::Resolved;
            if reopened {
                // A resolved insight firing again re-opens (count continues). Status → open; the
                // prior resolver/ts are cleared (a fresh open lifecycle).
                //
                // DO NOT clear `assigned_to` here, and do not touch the comment thread. This is the
                // single most revert-prone line in the record: it looks inconsistent (we just
                // cleared two other nullable fields) and it is not. `status_by`/`status_ts` describe
                // the LIFECYCLE that just ended, so a fresh open discards them; `assigned_to` and
                // the thread are HUMAN FACTS about the finding — the fault came back and it is still
                // Priya's, and the note explaining last time's false alarm is the most valuable
                // thing on this record at exactly this moment. Clearing them here is how a flapping
                // sensor silently un-assigns the technician who took the job.
                // SCOPE: docs/scope/insights/insight-triage-scope.md §"Intent / approach"
                //        ("The dedup rule is the load-bearing decision") + §"Testing plan" §1
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
                analysis: input.analysis.clone(),
                origin: input.origin.clone(),
                status: Status::Open,
                status_by: None,
                status_ts: None,
                // A brand-new finding is unassigned — it lands in the triage queue
                // (`insight.list { assigned_to: "none" }`) for a human to pick up. There is
                // deliberately no `assigned_to` on `RaiseInput`: a producer cannot reach the human
                // triage plane at all (insight-triage-scope.md §"Intent / approach").
                assigned_to: None,
                count: 1,
                first_ts: input.ts,
                last_ts: input.ts,
                producer: input.producer.clone(),
                // The tag echo is NOT written here — `input.tags` is one raise's DECLARATION, and
                // the echo must be the union across all raises, read back from the graph after the
                // host applies them (`insight-tag-echo-scope.md` §"Resolved decisions" 2). The host
                // layer writes it via `set_tags_echo` immediately after this call; on a dedup arm
                // the prior echo is carried through by the `prior` clone above.
                tags: std::collections::BTreeMap::new(),
                // Host-computed / derived below, uniformly for both arms — see the block after
                // this match. A caller cannot reach any of the four (no field on `RaiseInput`).
                caveats: Vec::new(),
                // The case back-ref is the GROUPING pass's to write, never raise's
                // (case-plane-scope.md resolved decision 4). A brand-new finding has no case yet.
                case_id: None,
                month_hist: empty_month_hist(),
                pattern: Pattern::New,
            };
            (insight, true, IntentKind::Raise)
        }
    };

    // --- Derived, every raise, both arms -------------------------------------------------------
    // The month counters and the pattern are computed from what is already on the record, so they
    // are identical for a create and a re-raise and there is no arm to get out of step. Bump FIRST,
    // then classify, so this firing counts toward its own classification.
    // SCOPE: docs/scope/insights/case-plane-scope.md §"Data model" (`month_hist`, `pattern`)
    bump_month(&mut insight.month_hist, input.ts);
    insight.pattern = derive_pattern(
        &insight.month_hist,
        insight.count,
        insight.first_ts,
        insight.last_ts,
    );

    // --- The caveat stamp ----------------------------------------------------------------------
    // Which open findings undermine this one? Only meaningful when (a) the workspace declared a
    // gating category and (b) this finding states the subjects it rests on. Both absent is the
    // common case today and costs one map lookup.
    //
    // A finding IN a gating category is never caveated by another one: a data-quality finding is
    // not softened by a second data-quality finding, and mutual caveating between two of them would
    // silence both. The check reads THIS raise's declared category (the echo is the host's to
    // materialize, after this call).
    // SCOPE: docs/scope/insights/case-plane-scope.md §"Data model" (`caveats`) + §"Testing plan"
    let gating = vocab
        .as_ref()
        .map(TagVocab::gating_values)
        .unwrap_or_default();
    let own_category = input.tags.get(CATEGORY_KEY).map(String::as_str);
    let self_gates = own_category.is_some_and(|c| gating.iter().any(|g| g == c));
    let subjects = insight
        .evidence
        .as_ref()
        .map(|e| e.subjects.clone())
        .unwrap_or_default();
    let mut caveats: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    if !self_gates && !subjects.is_empty() {
        for category in &gating {
            for id in caveats_for(store, ws, &subjects, &insight.id, category).await? {
                caveats.insert(id);
            }
        }
    }
    // REFRESHED on every raise (not merged): the caveat is a statement about the world RIGHT NOW,
    // so resolving the gating finding must clear it on the dependent finding's next firing. A merge
    // would make a caveat permanent, which is the failure mode that teaches operators to ignore it.
    insight.caveats = caveats.into_iter().collect();
    let caveated = !insight.caveats.is_empty();

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
        caveated,
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
