//! The raise-time matcher's `Intent` — what a match produces (insight-subscriptions-scope.md).
//!
//! The matcher is a PURE function `(insight_view, subs) -> Vec<Intent>` — no I/O. Each Intent is
//! handed to the notify engine (which owns all send/hold decisions + the actual `channel.post`
//! under the sub's stored principal). The `kind` drives the ladder's breakthrough checks (a
//! `Reopen` or escalation always delivers, regardless of the sub's current ladder level).

use serde::{Deserialize, Serialize};

use crate::severity::Severity;

/// What kind of raise produced this intent. Drives the ladder's breakthrough rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IntentKind {
    /// A first-ever raise of this `dedup_key` on this sub, or any raise that's not a re-open /
    /// escalation. The common case.
    Raise,
    /// The insight was `resolved` and is firing again — re-opened. Always breaks through.
    Reopen,
    /// This firing's severity is strictly higher than the previous on this key. Always breaks
    /// through (escalation is genuinely new information).
    Escalate,
}

/// One notification intent from the matcher → the notify engine. The engine owns delivery; this
/// only carries what the engine needs to decide now-vs-digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    /// The subscription that matched (the delivery destination + stored principal).
    pub sub_id: String,
    /// The insight that fired (the subject of the notification).
    pub insight_id: String,
    /// The insight's dedup key (the per-key ladder state key).
    pub dedup_key: String,
    /// This firing's severity (the escalation check + the digest's `max_severity` rollup).
    pub severity: Severity,
    /// What kind of raise — drives the breakthrough check.
    pub kind: IntentKind,
}
