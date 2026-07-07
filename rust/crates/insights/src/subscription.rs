//! `Subscription` — a member's filter + channel sink (insight-subscriptions-scope.md).
//!
//! A member subscribes a channel to all / a rule / an identity (`dedup_key`) / a tag facet / a
//! severity floor. The matcher evaluates subs at raise time and produces intents; the notify
//! engine delivers under the sub's stored principal, **re-checked at fire time** (the reminders
//! pattern). On a fire-time deny the sub flips to `muted` with a `DormantReason` and one final
//! system item is posted to the *owner's* inbox (never a silent stop).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::severity::Severity;

/// The store table subscriptions live in. One per workspace namespace.
pub const TABLE: &str = "insight_sub";

/// The hard workspace cap on subscriptions (the tags 10k-cap pattern). Create above this is
/// rejected as `BadInput`.
pub const MAX_PER_WORKSPACE: usize = 1_000;

/// The sink a subscription delivers into. v1 is channel-only (an `outbox` sink kind is additive
/// once the email `Target` exists — subscriptions scope non-goal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubSinkKind {
    Channel,
}

/// A subscription's delivery destination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubSink {
    /// The sink kind (v1: `Channel` only).
    pub kind: SubSinkKind,
    /// The target channel id. The owner must hold `bus:chan/{channel}:pub` at create AND fire.
    pub channel: String,
}

/// The AND-composed filter over insights. Every provided field must match; all absent = "all
/// insights in this workspace". Tag matching is a subset check against the insight's tag edges.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubFilter {
    /// Subscribe to one rule/flow (its `origin.ref`). Exact match v1 (glob is an open question).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin_ref: Option<String>,
    /// Subscribe to one identity (the insight's `dedup_key`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedup_key: Option<String>,
    /// Tag facets — the insight must carry ALL of these (`{ k: v, … }` → subset check).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tags: BTreeMap<String, String>,
    /// Severity floor — the insight must be at least this severe.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity_min: Option<Severity>,
}

/// Why a subscription went dormant (fire-time deny). Surfaced to the owner via an inbox note so
/// a sub never silently stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DormantReason {
    /// The owner left the workspace (membership removed).
    MemberRemoved,
    /// The channel `pub` grant was revoked.
    GrantRevoked,
    /// The channel itself was deleted.
    ChannelGone,
}

/// A subscription record. Stable on `id` (a ULID). `muted` is an owner toggle (keep the sub, stop
/// deliveries — accounting continues); `throttle_override` pins a ladder level (a pager channel
/// wants `Immediate` always). The owner + principal + dormant state live on this one row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    /// ULID, host-assigned at create.
    pub id: String,
    /// The owning member subject (`user:…`).
    pub owner: String,
    /// The stored principal snapshot, re-checked at fire (the reminders pattern). Opaque caps
    /// list the host re-runs against `bus:chan/{channel}:pub` at delivery.
    pub principal: serde_json::Value,
    /// The delivery sink.
    pub sink: SubSink,
    /// The AND filter (absent fields = "any").
    pub filter: SubFilter,
    /// Owner toggle — keep the sub, stop deliveries (notify state still accumulates).
    #[serde(default)]
    pub muted: bool,
    /// Pin a ladder level (skip escalate/decay; breakthroughs + ack-suppression still apply).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throttle_override: Option<crate::policy::ThrottleOverride>,
    /// Logical create timestamp.
    pub created_ts: u64,
    /// Why this sub went dormant (a fire-time deny flipped it). Absent ⇒ active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dormant_reason: Option<DormantReason>,
}
