//! `series.producer.health` — what the things WRITING a series say about their own ingest
//! (series-observability scope, slice D).
//!
//! `series.stats` already answers *who* writes a series and *when they last did*, generically, from
//! the host's own rows. What it cannot answer is anything only the producer knows: that its link is
//! reconnecting, that it has timed out eleven times in a row, that it is deliberately paused. This
//! verb asks them.
//!
//! # How the host reaches a producer without knowing any extension (rule 10)
//!
//! Two generic steps, neither of which names an extension:
//!
//! 1. **Producer → extension id is read out of the IDENTITY GRAMMAR.** `ingest.write` roots every
//!    stored producer at the authenticated principal (`{sub}/{declared}`), and an extension's
//!    principal sub is `ext:{id}` — so [`producer_ext_id`] recovers the id by shape. A series
//!    written by a human, an agent, an api key, a flow or a webhook simply yields `None`, which is a
//!    first-class answer here, not a failure.
//! 2. **Discovery is by TOOL-NAME CONVENTION over the live registry.** An extension contributes by
//!    declaring an ordinary tool named [`PRODUCER_HEALTH_TOOL`]; the host finds it in
//!    `registry.descriptor_entries()` exactly as `agent::exfil::tainted_tools` finds
//!    `emits_external` descriptors — a match on a self-declared property, with the ext id treated as
//!    opaque data. **No SDK change and no manifest change are involved**: an extension that wants to
//!    contribute declares one more tool, like any other.
//!
//! The host therefore knows a *convention* and an *identity shape*. It does not know that any
//! extension exists, and swapping every extension on the node changes nothing here.
//!
//! # The payload is the producer's, and the host does not interpret it
//!
//! Only three facts are modelled, and each is a property of *writing samples to ingest* rather than
//! of any particular kind of producer: when you last wrote, how many the host took, and what you
//! call the state you are in. Everything domain-specific — poll intervals, timeout counts, register
//! errors — rides in the open `details` list as label/value pairs the host passes through verbatim.
//! Modelling `consecutive_timeouts` as a host field would have quietly encoded "a producer is a
//! polling device", which is exactly the assumption a webhook or a flow breaks.
//!
//! # Refused, silent and broken are three different answers
//!
//! The scope's governing rule is that a fabricated-healthy panel is worse than no panel, so every
//! way of not knowing is reported distinctly and never as data: [`ProducerHealth::state`] separates
//! `not-an-extension` / `not-reported` / `denied` / `error` from `reported`. A denial in particular
//! is never folded into "this producer says nothing" — the caller's own
//! `mcp:{ext}.ingest.health:call` gate is what refused, and the panel must be able to name it.
//!
//! One producer's failure never fails the read: each row is resolved independently, so a broken
//! extension cannot blank the strip for the healthy one beside it.

use std::collections::BTreeMap;
use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::authorize::authorize_ingest;
use super::error::IngestError;
use super::write::{producer_ext_id, producer_leaf};
use crate::boot::Node;

/// The bare tool name an extension declares to report its own ingest health.
///
/// A CONVENTION, not a registry of extensions: the host matches this string against declared
/// descriptors and learns nothing about who declared it. Named `ingest.health` rather than a bare
/// `health` because an extension's *process* health is already reported by `ext.list`
/// (`running`/`restart_count`) — this is specifically the health of what it feeds into ingest, and
/// an extension may be perfectly alive while its ingest has stalled.
pub const PRODUCER_HEALTH_TOOL: &str = "ingest.health";

/// One label/value fact a producer reports that the host does not model.
///
/// Deliberately stringly-typed and self-describing: this is where a producer puts the things only it
/// understands, and the host's job is to carry them to a pixel without inventing meaning for them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducerDetail {
    pub label: String,
    pub value: String,
}

/// What a producer says about its own ingest. Every field is optional because a producer that does
/// not know a fact must be able to say so — an absent value renders "unknown" downstream and must
/// never be defaulted to `0`, which would read as a real measurement of zero.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProducerReport {
    /// The producer's own word for its current condition (e.g. `connected`, `reconnecting`,
    /// `paused`). NOT interpreted, NOT enumerated and NOT judged by the host — it is the producer's
    /// vocabulary, and a host that mapped it to a healthy/unhealthy verdict would be inventing the
    /// cadence knowledge the scope forbids.
    pub state: Option<String>,
    /// Epoch ms when this producer last successfully handed samples to ingest.
    pub last_write_ms: Option<u64>,
    /// How many samples the host accepted on that write.
    pub last_accepted: Option<u64>,
    /// How long that write took, in ms.
    ///
    /// Modelled — rather than left to `details` — because WITHOUT it `last_accepted: 0` is two
    /// opposite conditions sharing one number, and no consumer can tell them apart:
    ///
    /// - a producer that had nothing to send (a change-of-value filter suppressed everything, a
    ///   quiet interval) reports `last_accepted: 0` with a near-zero duration. It is **healthy**.
    /// - a producer whose write FAILED or timed out reports `last_accepted: 0` after a long
    ///   duration. It is **losing data**.
    ///
    /// Rendering those identically is what lets total data loss look like a quiet meter — the
    /// observed failure in `rubix-ai/docs/debugging/2026-08-04-dead-producer-epochs-render-connected.md`.
    /// It stays `Option` like everything else here: a producer that does not measure it says so, and
    /// absence must never be defaulted to `0` (which would claim an instant write that never happened).
    ///
    /// This is a fact about ANY producer of samples — a webhook's delivery, a flow's batch write —
    /// not a polling-specific one, which is what earns it a modelled field rather than a detail row.
    pub last_push_ms: Option<u64>,
    /// Whether this producer is still the CURRENT generation of its stream.
    ///
    /// A producer identity may carry a generation (an epoch, a spawn id, a connection leg). When a
    /// producer restarts, its earlier identities remain in the store as authors of historical
    /// samples, and a consumer asking about one of them is asking about a stream that has ENDED.
    ///
    /// `Some(false)` says exactly that, and is the difference between "this stream is dead" and the
    /// live stream's status wearing a dead stream's name. `None` means the producer does not model
    /// generations at all (the common case — most producers have exactly one).
    ///
    /// The host neither parses nor assigns generations: only the producer knows whether the identity
    /// it was handed is its current one.
    pub is_current: Option<bool>,
    /// Everything else the producer wants shown, in its own words.
    pub details: Vec<ProducerDetail>,
}

/// Why a producer's row reads the way it does. Serialized kebab-case as `state`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthState {
    /// The producer is not an extension (a human, agent, api key, flow or webhook). There is
    /// nothing to ask and nothing is wrong — most series in a workspace look like this.
    NotAnExtension,
    /// The producer IS an extension, but it declares no `ingest.health` tool on this node. It has
    /// opted out of the convention, which is allowed.
    NotReported,
    /// The caller may not call this extension's health tool. NOT the same as "reports nothing":
    /// something is there and we were refused it.
    Denied,
    /// The call was made and failed. The message is the host's, never fabricated.
    Error,
    /// The producer answered.
    Reported,
}

/// One row of the producer strip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducerHealth {
    /// The stored producer id, exactly as it appears on the samples and in `series.stats`.
    pub producer: String,
    /// The extension behind it, when there is one.
    pub ext: Option<String>,
    pub state: HealthState,
    /// Present only for `Reported`.
    pub report: Option<ProducerReport>,
    /// Present only for `Error` — the real failure text, so a broken producer is diagnosable
    /// instead of merely blank.
    pub message: Option<String>,
    /// The exact capability the caller was refused. Present only for `Denied`, so the panel can name
    /// the missing grant rather than saying "not available" and leaving the operator to guess.
    pub missing_cap: Option<String>,
}

impl ProducerHealth {
    fn plain(producer: &str, ext: Option<&str>, state: HealthState) -> Self {
        Self {
            producer: producer.to_string(),
            ext: ext.map(str::to_string),
            state,
            report: None,
            message: None,
            missing_cap: None,
        }
    }
}

/// The verb's result. A list, because a series is written by a SET of producers — the multi-producer
/// case is the normal case, not an edge case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeriesProducerHealth {
    pub series: String,
    pub producers: Vec<ProducerHealth>,
}

/// Ask every producer of `series` about its own ingest health.
///
/// Gated by `mcp:series.producer.health:call` (the data-plane tier, beside `series.stats` — knowing
/// who writes a series and whether they are healthy is the same class of read as knowing how many
/// samples they wrote). Each per-extension call is then gated AGAIN by the caller's own
/// `mcp:{ext}.ingest.health:call`, under the caller's own principal: this verb is a fan-out
/// convenience, never a privilege escalation, and a caller cannot reach through it to an extension
/// tool it could not call directly.
pub async fn series_producer_health(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    series: &str,
    depth: u32,
) -> Result<SeriesProducerHealth, IngestError> {
    authorize_ingest(principal, ws, "series.producer.health")?;

    let producers = lb_ingest::series_producers(&node.store, ws, series).await?;
    // Which extensions declare the convention, resolved ONCE for the whole fan-out rather than per
    // producer — one extension commonly writes several streams of a series.
    let declaring = declaring_extensions(node);

    let mut rows = Vec::with_capacity(producers.len());
    for producer in producers {
        rows.push(resolve_one(node, principal, ws, series, &producer, &declaring, depth).await);
    }

    Ok(SeriesProducerHealth {
        series: series.to_string(),
        producers: rows,
    })
}

/// The set of installed extensions that declare [`PRODUCER_HEALTH_TOOL`], from the live registry.
///
/// Mirrors `agent::exfil::tainted_tools`: walk every registered extension, match a self-declared
/// descriptor property, and treat the ext id as opaque. The registry is the truthful source here —
/// it lists what is actually dispatchable on this node right now, so a declared-but-unloaded
/// extension correctly reads as "not reported" instead of erroring on a call that cannot land.
fn declaring_extensions(node: &Node) -> BTreeMap<String, ()> {
    node.registry
        .descriptor_entries()
        .into_iter()
        .filter(|(_, descriptors)| descriptors.iter().any(|d| d.name == PRODUCER_HEALTH_TOOL))
        .map(|(ext_id, _)| (ext_id, ()))
        .collect()
}

/// Resolve ONE producer's row. Never returns an error: a producer that cannot be reached is a state
/// on its own row, so one broken extension cannot blank the strip for the healthy one beside it.
async fn resolve_one(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    series: &str,
    producer: &str,
    declaring: &BTreeMap<String, ()>,
    depth: u32,
) -> ProducerHealth {
    let Some(ext) = producer_ext_id(producer) else {
        return ProducerHealth::plain(producer, None, HealthState::NotAnExtension);
    };
    if !declaring.contains_key(ext) {
        return ProducerHealth::plain(producer, Some(ext), HealthState::NotReported);
    }

    let qualified = format!("{ext}.{PRODUCER_HEALTH_TOOL}");
    // Hand the producer back its OWN stream id (the leaf it declared — it never saw the root we
    // stamped) plus the series being asked about, so an extension feeding many streams can answer
    // for the one in question instead of averaging itself into uselessness.
    let args = json!({ "producer": producer_leaf(producer), "series": series }).to_string();

    // Re-enter the one dispatch chokepoint at depth+1 under the CALLER's principal: the workspace
    // wall and the extension tool's own capability are both re-checked, and `Box::pin` because the
    // extension's callback may re-enter this dispatcher (the `viz`/`nav` precedent).
    let called = Box::pin(crate::tool_call::call_tool_at_depth(
        node,
        principal,
        ws,
        &qualified,
        &args,
        depth + 1,
    ))
    .await;

    match called {
        Ok(body) => match serde_json::from_str::<ProducerReport>(&body) {
            Ok(report) => ProducerHealth {
                producer: producer.to_string(),
                ext: Some(ext.to_string()),
                state: HealthState::Reported,
                report: Some(report),
                message: None,
                missing_cap: None,
            },
            // A producer that answers in a shape we cannot read is BROKEN, not silent — saying
            // "reports nothing" here would hide a contract bug behind a plausible-looking blank.
            Err(e) => ProducerHealth {
                message: Some(format!("unreadable {PRODUCER_HEALTH_TOOL} reply: {e}")),
                ..ProducerHealth::plain(producer, Some(ext), HealthState::Error)
            },
        },
        Err(ToolError::Denied) => ProducerHealth {
            missing_cap: Some(format!("mcp:{qualified}:call")),
            ..ProducerHealth::plain(producer, Some(ext), HealthState::Denied)
        },
        Err(e) => ProducerHealth {
            message: Some(format!("{e:?}")),
            ..ProducerHealth::plain(producer, Some(ext), HealthState::Error)
        },
    }
}
