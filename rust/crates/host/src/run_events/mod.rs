//! The **run-event stream** host surface (agent-run scope Part 3) — the live feed a run projects so
//! the UI (and ACP) observe it, built on the Part-1 [`RunEvent`](lb_run_events) vocabulary and the
//! Part-0 durable transcript.
//!
//! This is the **start/resume vs watch split** in practice: driving a run (`invoke`/`resume`) is one
//! path; *observing* it is this one. A lifecycle client (the ACP adapter, the browser) drives with
//! start/resume and watches here, instead of being forced through a blocking final-answer call.
//! `agent.invoke` stays only as a compatibility wrapper for plain MCP callers (it starts + blocks
//! for the answer).
//!
//! Verbs, one responsibility per file (FILE-LAYOUT §3):
//! - `subject` — the bus key the stream rides (workspace-walled by `lb_bus`).
//! - `publish` — the loop emits one `RunEvent` (best-effort motion, after the durable append).
//! - `watch` — `agent.watch`: a transcript snapshot + the live delta subscription.
//! - `stream` — the typed subscription wrapper the SSE/ACP encoders consume.

mod control;
mod publish;
mod stream;
mod subject;
mod watch;

pub use control::{pause_run, resume_run, stop_run, AGENT_CONTROL_TOOL};
pub use publish::publish_run_event;
pub use stream::RunEventSub;
pub use subject::run_subject;
pub use watch::{watch_run, RunWatch};
