//! The `Launcher` seam — *how a child is spawned*, abstracted so the supervisor logic is testable
//! without a real OS process for the deny/isolation/unit paths, while the supervision-restart proof
//! uses the real OS launcher (native-tier scope: mock only the true external; a real process IS the
//! external, so the restart proof uses [`os::OsLauncher`], the unit paths use a fake).
//!
//! A launch yields a [`Channel`]: a framed write half (supervisor → child requests), a framed read
//! half (child → supervisor replies), and a [`Kill`] handle to terminate the child and await its
//! exit. The trait is the only thing the `Sidecar` depends on, so a respawn is "ask the launcher for
//! a fresh channel" — identical for an OS child and a fake.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::SupervisorError;

/// The write half of a child's control line (the supervisor writes framed requests here).
pub type ChildWrite = Pin<Box<dyn AsyncWrite + Send + Unpin>>;
/// The read half of a child's control line (the supervisor reads framed replies here).
pub type ChildRead = Pin<Box<dyn AsyncRead + Send + Unpin>>;

/// A terminate handle for a launched child. `kill` ends the child (process-group kill for the OS
/// launcher) and resolves when it has exited — so a respawn cannot race a still-living predecessor.
pub trait Kill: Send {
    fn kill(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>>;
}

/// One launched child: the two framed halves plus its kill handle.
pub struct Channel {
    pub write: ChildWrite,
    pub read: ChildRead,
    pub kill: Box<dyn Kill>,
}

/// How to (re)spawn a child for a [`Spec`](crate::spec::Spec). The `env` passed to `launch` is the
/// spec's base env merged with the host-injected identity (`LB_EXT_*`) — the launcher just applies
/// it. Implementors: [`os::OsLauncher`] (real process) and test fakes.
pub trait Launcher: Send + Sync {
    /// Spawn `exec args…` with environment `env`, returning the child's control channel.
    fn launch(
        &self,
        exec: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> impl Future<Output = Result<Channel, SupervisorError>> + Send;
}
