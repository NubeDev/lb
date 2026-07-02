//! The reusable poll engine (slice 3). Split by responsibility (FILE-LAYOUT):
//!
//! - [`source`] — the driver-agnostic `Source` read seam + `PollTarget`/`Reading` shapes.
//! - [`gating`] — the `enable`-AND rule (connection ∧ network ∧ device ∧ point), a pure fn + tests.
//! - [`sink`] — the `Sink` write seam; `IngestSink` wraps `ingest.write` via the host callback.
//! - [`poller`] — the tick core (`poll_once`) + `Backoff`, both pure w.r.t. time and unit-tested.
//! - [`run`] — the async poll task + `PollRegistry` (the `ros.start|stop|status` runnable machinery).
//! - [`ros_source`] — the ONE ROS-specific adapter (`RosApi` → `Source`, series id, present_value).
//! - [`ros_target`] — the ROS-specific delivery adapter for `point.write` outbox effects (slice 4).
//! - [`relay`] — the sidecar relay loop that delivers `ros` outbox effects via `ros_target` (slice 4).
//!
//! The poll engine (`source`/`gating`/`sink`/`poller`/`run`) is driver-agnostic and reusable; the ROS
//! specifics are isolated to `ros_source` (reads) and `ros_target` (writes). The engine
//! (loop/gating/backoff/batch) is provable with a stub `Source` — no box, no gateway (see
//! `poller::tests`).

pub mod gating;
pub mod poller;
pub mod relay;
pub mod ros_source;
pub mod ros_target;
pub mod run;
pub mod sink;
pub mod source;
