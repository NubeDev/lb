//! The `reminders:` block of a pack manifest — a dependency-free MIRROR of the `reminder.create`
//! verb args (`lb_reminders::{Reminder, Action}`), so a reminder that validates in a pack is
//! byte-for-byte the one the verb takes.
//!
//! **Why a mirror and not the real types.** Same layering reason as [`crate::manifest_retention`]:
//! `lb-packs` is the pure, dependency-light half of the pack engine, and `lb-reminders` depends on
//! `lb-store`. Reusing `Action` would drag the store into the manifest crate to reuse one enum. The
//! cost is that `action_kind` is a `String` rather than the real tag, so [`crate::validate_reminder`]
//! carries the lint that rejects an unknown kind at validate time — where the author is still
//! looking — instead of letting the apply-side conversion fail opaquely (the closed-struct trap).
//!
//! **Why a pack may declare a schedule at all.** Every other pack section seeds a thing that sits
//! there until something drives it: a rule is inert until `rules.run`, a channel until someone
//! posts. A pack that ships seven FDD rules and no schedule ships a product that detects nothing
//! until an operator wires the cron by hand, off-manifest and undocumented — which is exactly what
//! the rubix-ai BMS duty cycle had to do in a shell script. The schedule is as much a part of the
//! product as the rule it fires.
//!
//! `deny_unknown_fields` throughout: a typo'd key is a loud parse error, never a swallowed line.

use serde::{Deserialize, Serialize};

/// One reminder to seed (`pack-core-scope`, the `reminders:` block). Mirrors the `reminder.create`
/// args; the apply arm converts this straight into `lb_reminders::Action` + a `reminder_create`
/// call. Keyed by `id` (its natural id — a reminder row is one-per-id, LWW upsert).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PackReminder {
    /// Workspace-unique, stable. Also the receipt object id (`reminder:<id>`). Re-applying upserts
    /// the same row, so an edited schedule lands on the next apply rather than forking a second
    /// reminder — the LWW model every other inline pack object uses.
    pub id: String,
    /// The 5-field cron schedule. Validated at apply time by `reminder.create` itself (`is_valid`),
    /// and shape-linted here so a bad field count is caught with the author still looking.
    pub schedule: String,
    /// Which action shape this fires: `channel-post`, `mcp-tool` or `outbox`. A `String` for the
    /// mirror reason above; `validate_reminder` rejects an unknown name against the closed set.
    pub action_kind: String,
    /// `channel-post`: the channel to post into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// `channel-post`: the message body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// `mcp-tool`: the verb to call. Fires through the host `call_tool` chokepoint, which re-checks
    /// `mcp:{tool}:call` under the STORED principal at fire time — a pack-seeded reminder is not a
    /// privileged one (`pack/mod.rs` §No cap smuggling).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// `mcp-tool`: the verb's args, passed through verbatim at each firing.
    ///
    /// **`{{fire_ts}}`.** Reminder args are stored verbatim, so a verb that is idempotent on a
    /// caller-supplied id replays its first result forever. A `{{fire_ts}}` token in any string
    /// value is substituted with the fire clock's epoch seconds (`reminder/range.rs`), which is how
    /// a scheduled `agent.invoke` varies its `job_id`. `validate_reminder` warns when it is absent
    /// from an args block that carries a `job_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
    /// `outbox`: the delivery target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// `outbox`: the action name on that target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// `outbox`: the effect payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
    /// `Some(n)` (n ≥ 1) = fire at most `n` times then stop; absent = recurring forever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_runs: Option<u32>,
}

/// The closed set of action kinds, mirroring `lb_reminders::Action`'s serde tags.
pub const ACTION_KINDS: [&str; 3] = ["channel-post", "mcp-tool", "outbox"];
