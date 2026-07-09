//! The **subject registry** — the one place that maps an opaque mux subject string onto the SAME
//! `lb_host` gate + snapshot + live feed the dedicated per-feature SSE route uses (unified-event-stream
//! scope §3). A subject is `kind:id` (flat strings, opaque to the mux — Open-Q "lean flat"), and each
//! kind calls its existing host verb UNCHANGED:
//!
//! | subject          | host verb (gate)                                    | cap                       |
//! |------------------|-----------------------------------------------------|---------------------------|
//! | `run:{job}`      | `watch_run` (snapshot + deltas)                     | `mcp:agent.watch:call`    |
//! | `channel:{cid}`  | `subscribe_channel`+`watch_deletions`+`watch`       | channel `sub` grant       |
//! | `series:{s}`     | `subscribe_series`                                  | `mcp:series.read:call`    |
//! | `bus:{subject}`  | `bus_watch` (subject walled host-side)              | `mcp:bus.watch:call`      |
//! | `flow-run:{run}` | `watch_flow_run` (snapshot + deltas)                | `mcp:flows.watch:call`    |
//! | `flow-debug:{f}` | `watch_flow_debug` (deltas-only)                    | `mcp:flows.debug.watch:call` |
//! | `insights`       | `subscribe_insight_events`                          | `mcp:insight.watch:call`  |
//! | `telemetry`      | `telemetry_tail` (snapshot + deltas)                | `mcp:telemetry.read:call` |
//!
//! The gate is NEVER re-implemented here: a deny from the host verb becomes a per-subject error frame
//! (the caller emits it), and the connection lives on. The workspace is the connection's (`principal.ws()`),
//! so a subject naming another workspace's id is the same opaque deny as an unknown subject — the mux is
//! not an existence oracle (scope "Tenancy / isolation").
//!
//! Each kind's heterogeneous handle (`RunWatch`, `BusSub`, `PresenceFeed`, …) is adapted into ONE boxed
//! `Stream<Item = SubjectFrame>` so the connection task folds every subject the same way. The `(event, data)`
//! pair is byte-identical to what the dedicated route emits (scope "Frame-shape compatibility").

use std::pin::Pin;

use futures::stream::{Stream, StreamExt};
use lb_auth::Principal;
use serde_json::json;

use crate::state::Gateway;

/// One frame a subject produces: the original SSE `event:` name + its JSON `data` payload (already a
/// string, exactly as the dedicated route would serialize it). The mux envelope wraps this verbatim.
pub struct SubjectFrame {
    pub event: String,
    pub data: String,
}

/// A subject's live frame stream — the boxed, uniform feed the connection task folds. Producing it ran
/// the subject's gate already (a gate failure is [`SubjectError`], never a stream).
pub type SubjectStream = Pin<Box<dyn Stream<Item = SubjectFrame> + Send>>;

/// Why a subscribe failed. Both collapse to the same opaque per-subject error frame on the wire (no
/// oracle: an unknown subject and a denied/cross-workspace one are indistinguishable — scope tenancy).
#[derive(Debug)]
pub enum SubjectError {
    /// The subject string didn't parse to a known kind.
    Unknown,
    /// The subject's host gate denied (missing cap, cross-workspace, or a subject-wall refusal).
    Denied,
}

/// Resolve `subject` for `principal` (in the principal's workspace): run its host gate + snapshot read,
/// and return the boxed live frame stream. This is the mux's single seam onto the dedicated routes'
/// logic — every arm calls the exact host verb its route calls, so the cap check cannot drift.
pub async fn open_subject(
    gw: &Gateway,
    principal: &Principal,
    subject: &str,
) -> Result<SubjectStream, SubjectError> {
    let ws = principal.ws().to_string();
    let (kind, id) = split_subject(subject);
    match kind {
        "run" => {
            let watch = lb_host::watch_run(&gw.node.store, &gw.node.bus, principal, &ws, id)
                .await
                .map_err(|_| SubjectError::Denied)?;
            // Phase 1: the transcript snapshot, then Phase 2: live deltas — same order as `run_stream`.
            let snapshot =
                futures::stream::iter(watch.snapshot.into_iter()).map(|ev| frame("run", &ev));
            let live = futures::stream::unfold(watch.stream, |sub| async move {
                sub.recv().await.map(|ev| (frame("run", &ev), sub))
            });
            Ok(Box::pin(snapshot.chain(live)))
        }
        "channel" => {
            // Three feeds merged into one stream, exactly as `channel_stream` (message/delete/presence).
            let sub = lb_host::subscribe_channel(&gw.node.bus, principal, &ws, id)
                .await
                .map_err(|_| SubjectError::Denied)?;
            let deletions = lb_host::watch_deletions(&gw.node.bus, principal, &ws, id)
                .await
                .map_err(|_| SubjectError::Denied)?;
            let presence = lb_host::watch(&gw.node.bus, principal, &ws, id)
                .await
                .map_err(|_| SubjectError::Denied)?;
            let stream = futures::stream::unfold(
                (sub, deletions, presence),
                |(sub, deletions, presence)| async move {
                    let frame = tokio::select! {
                        item = sub.recv() => item.map(|i| frame("message", &i)),
                        did = deletions.recv() => did.map(|did| raw_frame("delete", json!({ "id": did }))),
                        change = presence.recv() => change
                            .map(|(member, present)| raw_frame("presence", json!({ "member": member, "present": present }))),
                    };
                    frame.map(|f| (f, (sub, deletions, presence)))
                },
            );
            Ok(Box::pin(stream))
        }
        "series" => {
            let sub = lb_host::subscribe_series(&gw.node.bus, principal, &ws, id)
                .await
                .map_err(|_| SubjectError::Denied)?;
            let stream = futures::stream::unfold(sub, |sub| async move {
                sub.recv()
                    .await
                    .map(|sample| (frame("sample", &sample), sub))
            });
            Ok(Box::pin(stream))
        }
        "bus" => {
            let sub = lb_host::bus_watch(&gw.node.bus, principal, &ws, id)
                .await
                .map_err(|_| SubjectError::Denied)?;
            let stream = futures::stream::unfold(sub, |sub| async move {
                sub.recv().await.map(|bytes| {
                    let value: serde_json::Value =
                        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
                    (raw_frame("message", value), sub)
                })
            });
            Ok(Box::pin(stream))
        }
        "flow-run" => {
            let watch = lb_host::watch_flow_run(&gw.node.store, &gw.node.bus, principal, &ws, id)
                .await
                .map_err(|_| SubjectError::Denied)?;
            let snapshot = futures::stream::once({
                let snap = watch.snapshot;
                async move { raw_frame("snapshot", snap) }
            });
            let live = futures::stream::unfold(watch.stream, |sub| async move {
                sub.recv().await.map(|ev| (raw_frame("flow", ev), sub))
            });
            Ok(Box::pin(snapshot.chain(live)))
        }
        "flow-debug" => {
            let watch = lb_host::watch_flow_debug(&gw.node.store, &gw.node.bus, principal, &ws, id)
                .await
                .map_err(|_| SubjectError::Denied)?;
            let stream = futures::stream::unfold(watch.stream, |sub| async move {
                sub.recv().await.map(|ev| (raw_frame("debug", ev), sub))
            });
            Ok(Box::pin(stream))
        }
        "insights" => {
            let sub = lb_host::subscribe_insight_events(&gw.node.bus, principal, &ws)
                .await
                .map_err(|_| SubjectError::Denied)?;
            let stream = futures::stream::unfold(sub, |sub| async move {
                sub.recv().await.map(|ev| (frame("message", &ev), sub))
            });
            Ok(Box::pin(stream))
        }
        "telemetry" => {
            let (snapshot, sub) =
                lb_host::telemetry_tail(&gw.node.store, &gw.node.bus, principal, &ws, 100)
                    .await
                    .map_err(|_| SubjectError::Denied)?;
            let snap =
                futures::stream::iter(snapshot.rows.into_iter()).map(|row| frame("snapshot", &row));
            let live = futures::stream::unfold(sub, |sub| async move {
                sub.recv().await.map(|bytes| {
                    let data = String::from_utf8(bytes).unwrap_or_default();
                    (
                        SubjectFrame {
                            event: "telemetry".into(),
                            data,
                        },
                        sub,
                    )
                })
            });
            Ok(Box::pin(snap.chain(live)))
        }
        _ => Err(SubjectError::Unknown),
    }
}

/// Split `kind:id` on the FIRST colon. A no-colon subject (`insights`, `telemetry`) is all-kind, empty
/// id. A `bus:` subject's id keeps its own `/`s and inner colons (the host walls it) — hence first-colon.
fn split_subject(subject: &str) -> (&str, &str) {
    match subject.split_once(':') {
        Some((kind, id)) => (kind, id),
        None => (subject, ""),
    }
}

/// Encode a serializable value as a `(event, data)` frame — the JSON is the dedicated route's payload
/// verbatim. A serialization failure degrades to an empty object (never panics the connection).
fn frame<T: serde::Serialize>(event: &str, value: &T) -> SubjectFrame {
    SubjectFrame {
        event: event.into(),
        data: serde_json::to_string(value).unwrap_or_else(|_| "{}".into()),
    }
}

/// Encode an already-built `serde_json::Value` as a frame.
fn raw_frame(event: &str, value: serde_json::Value) -> SubjectFrame {
    SubjectFrame {
        event: event.into(),
        data: value.to_string(),
    }
}
