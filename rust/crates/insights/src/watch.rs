//! `watch` — the fire-and-forget bus event subjects + payload shapes for `insight.watch`
//! (insights umbrella scope).
//!
//! The watch subject is workspace-scoped: `ws/{ws}/insight/events`. Fire-and-forget — a durable
//! consumer (the page) scans the table; the bus event only advances a live UI. Must-deliver
//! external notification stays the outbox's job (state vs motion, README §3). The host publishes
//! the event after the raise/ack/resolve write; the gateway's SSE route subscribes a browser.

use serde::{Deserialize, Serialize};

use crate::severity::Severity;
use crate::status::Status;

/// The bus subject for a workspace's insight events. WS-scoped (no cross-ws leak — the host
/// walls every publish). The gateway's `GET /insights/events` SSE subscribes this.
pub fn event_subject(ws: &str) -> String {
    format!("ws/{ws}/insight/events")
}

/// What kind of lifecycle event the bus carries. Drives the live UI's badge + list refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    /// A new insight was raised, or an existing one re-fired / re-opened.
    Raise,
    /// An insight was acked.
    Ack,
    /// An insight was resolved.
    Resolve,
}

/// The event payload published on `event_subject(ws)`. Lite — the UI fetches the full record via
/// `insight.get` if it needs detail; the bus carries only what a live list needs to update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaiseEvent {
    pub kind: EventKind,
    /// The insight id.
    pub id: String,
    /// The dedup key (so a live UI can coalesce events per identity).
    pub dedup_key: String,
    /// Post-event status.
    pub status: Status,
    /// Post-event severity.
    pub severity: Severity,
    /// Post-event lifetime count.
    pub count: u64,
    /// Logical ts of the event.
    pub ts: u64,
}
