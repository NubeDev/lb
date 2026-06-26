//! The real OS [`Launcher`] — spawn a child process with `tokio::process` and talk to it over its
//! piped stdin/stdout (native-tier scope). This is the one place that touches the OS-process
//! boundary; everything above it is process-agnostic (it only sees the `Launcher`/`Channel` seam).
//!
//! Isolation posture (the "minimal proven sidecar" decision): the child is spawned in **its own
//! process group** (`process_group(0)` on Unix) so a stop/restart can kill the whole group, not just
//! the immediate PID — a child that forked grandchildren is fully reaped, no zombies. Deeper OS
//! hardening (cgroups/seccomp/userns) is a noted follow-up, not this slice (native-tier non-goal).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;

use tokio::process::{Child, Command};

use crate::error::SupervisorError;
use crate::launcher::{Channel, Kill, Launcher};

/// Spawns real OS children. The host wires one of these into the supervisor for production; tests
/// use it only for the restart proof (a real process is the external being tested).
#[derive(Default)]
pub struct OsLauncher;

impl Launcher for OsLauncher {
    async fn launch(
        &self,
        exec: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Channel, SupervisorError> {
        let mut cmd = Command::new(exec);
        cmd.args(args)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        own_process_group(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| SupervisorError::Spawn(format!("{exec}: {e}")))?;

        let write = child
            .stdin
            .take()
            .ok_or_else(|| SupervisorError::Spawn("child has no stdin".into()))?;
        let read = child
            .stdout
            .take()
            .ok_or_else(|| SupervisorError::Spawn("child has no stdout".into()))?;

        Ok(Channel {
            write: Box::pin(write),
            read: Box::pin(read),
            kill: Box::new(OsKill { child }),
        })
    }
}

/// Put the child in its own process group so the kill targets the group (reaping any grandchildren).
#[cfg(unix)]
fn own_process_group(cmd: &mut Command) {
    cmd.process_group(0);
}
#[cfg(not(unix))]
fn own_process_group(_cmd: &mut Command) {}

/// The terminate handle for an OS child: kill the process (group via `kill_on_drop` + start_kill)
/// and await its exit so a respawn cannot overlap a living predecessor.
struct OsKill {
    child: Child,
}

impl Kill for OsKill {
    fn kill(mut self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move {
            let _ = self.child.start_kill();
            let _ = self.child.wait().await;
        })
    }
}
