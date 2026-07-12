//! The native **Tier-2 supervisor** — spawn and keep an OS child process alive (README §6.3 Tier 2,
//! native-tier scope). The peer of the Tier-1 wasm runtime (`lb-runtime`): where that loads a
//! component in-process, this supervises a real child binary — a language server, an MQTT bridge,
//! any extension that needs its own socket/thread/long-lived daemon.
//!
//! Like `lb-registry`/`lb-outbox`/`lb-jobs`, this crate holds **no store, no authorization, no
//! identity** — it is the OS plumbing + the supervision policy, behind a [`Launcher`] seam (the
//! registry's `Source` analogue). The host `native` service drives it: it computes the grant, mints
//! the child's scoped token, persists the durable record, and *then* asks this crate to spawn. That
//! keeps capability-first (§3.5: the host is the chokepoint) and keeps supervision **stateless** —
//! the live [`Sidecar`] is runtime-only; the durable truth is the host's record.
//!
//! One responsibility per file (FILE-LAYOUT):
//!   - `spec`     — the supervision recipe ([`Spec`], [`RestartPolicy`], [`Backoff`]).
//!   - `rpc`      — the child wire protocol shapes (`init`/`health`/`call`/`shutdown`).
//!   - `frame`    — `Content-Length` JSON-RPC framing over the child's stdio.
//!   - `launcher` — the [`Launcher`] seam + [`Channel`]/[`Kill`] (spawn behind a trait, testable).
//!   - `os`       — the real OS launcher ([`OsLauncher`], the one process-boundary file).
//!   - `sidecar`  — the live [`Sidecar`]: handshake · call · health · shutdown · restart.

mod error;
mod frame;
mod launcher;
mod os;
mod rpc;
mod sidecar;
mod spec;

pub use error::SupervisorError;
pub use frame::{read_frame, write_frame, MAX_FRAME};
pub use launcher::{Channel, ChildRead, ChildWrite, Kill, Launcher};
pub use os::OsLauncher;
pub use rpc::{CallParams, Caller, Method, Reply, Request};
pub use sidecar::Sidecar;
pub use spec::{Backoff, RestartPolicy, Spec};
