//! `react_to_profiles` — the freshness reactor for datasource discovery profiles
//! (datasource-profile scope), at the `react_to_reminders` / `relay_outbox` altitude: a stateless,
//! durable, workspace-isolated pass on a tick.
//!
//! Two jobs per tick, both bounded:
//!   1. **Enqueue** — find profiles older than `refresh_after_secs` and queue a rebuild.
//!   2. **Drain** — run the queued rebuilds through the same host path a manual call takes.
//!
//! **Scan cost is the standing hazard.** The stale query is index-backed AND `LIMIT`-bounded
//! (`profile_record::stale`), and the tick is lazy — minutes, not seconds. A full-table rescan on
//! every tick is exactly what pegged a Pi's CPU
//! (`docs/debugging/jobs/node-pegs-cpu-reactor-rescans-job-table.md`); this reactor must never
//! reintroduce it.
//!
//! **Off by default.** Compiled only with the `datasource-profile` cargo feature, and even then it
//! runs only when `BootConfig::profile` is present and enabled — role is config, never a code branch.
//!
//! **Workspace isolation.** Every read and write is namespaced to the `ws` it was called with; a
//! ws-B tick can no more touch a ws-A record than a ws-B caller can.

use std::sync::Arc;
use std::time::Duration;

use lb_auth::Principal;
use lb_jobs::{create, pending, JobStatus};
use lb_supervisor::OsLauncher;

use super::error::FederationError;
use super::net::FEDERATION_EXT;
use super::profile::{federation_profile, ProfileBounds};
use super::profile_record::{put, resolve as resolve_profile, stale, PROFILING_GUARD_SECS};
use super::profile_refresh::{profile_job_id, PROFILE_JOB_KIND};
use crate::boot::Node;

/// How many stale profiles one tick may enqueue, and how many queued jobs it may run. The cap is the
/// point: a workspace that suddenly has 10 000 stale profiles does bounded work per tick and catches
/// up over several ticks, rather than opening 10 000 external connections at once.
pub const MAX_PER_TICK: usize = 8;

/// The default lazy tick. Minutes, not seconds — a profile's freshness contract is measured in
/// hours, so polling faster buys nothing and costs the small-box CPU budget.
pub const PROFILE_PERIOD: Duration = Duration::from_secs(300);

/// The runtime knobs, supplied by `BootConfig`. Role = config; there is no `if cloud` here.
#[derive(Debug, Clone, Copy)]
pub struct ProfileReactorConfig {
    pub refresh_after_secs: u64,
    pub bounds: ProfileBounds,
}

/// What one pass did — the reactor's observable result, and what the tests assert on.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ProfilePass {
    /// Stale profiles for which a job was newly created this tick.
    pub enqueued: usize,
    /// Stale profiles skipped because a pass is already queued or in flight (the idempotence guard).
    pub skipped: usize,
    /// Queued jobs actually run to completion this tick.
    pub ran: usize,
    /// Jobs that failed; the record keeps its previous (stale) profile rather than losing it.
    pub failed: usize,
}

/// The principal a background pass runs as: the federation extension itself, in THIS workspace,
/// carrying exactly the read cap the pass needs. Never the caller's identity (there is no caller —
/// a clock fired), and never a broader grant. Workspace stays the hard wall.
fn reactor_principal(ws: &str) -> Principal {
    Principal::routed(
        format!("ext:{FEDERATION_EXT}"),
        ws.to_string(),
        vec![
            // The read privilege the pass itself needs…
            "mcp:federation.query:call".to_string(),
            // …and the authority to reach the supervised sidecar that performs it. Both are the
            // federation extension's OWN authority, not a caller's — there is no caller here, a
            // clock fired. Exactly these two and nothing more: a background pass must not be able
            // to do anything a `federation.profile` call could not.
            "mcp:native.call:call".to_string(),
        ],
    )
}

/// One reactor pass over `ws` at logical time `now`.
pub async fn react_to_profiles(
    node: &Arc<Node>,
    ws: &str,
    now: u64,
    cfg: ProfileReactorConfig,
) -> Result<ProfilePass, FederationError> {
    let mut pass = ProfilePass::default();

    // ── 1. enqueue the stale ────────────────────────────────────────────────────────────────────
    let cutoff = now.saturating_sub(cfg.refresh_after_secs);
    for rec in stale(&node.store, ws, cutoff, MAX_PER_TICK).await? {
        // In-flight guard. `profiling_since` is stamped when a pass starts and cleared when it
        // lands, so a long pass does not get re-enqueued on every tick for its whole duration. The
        // guard EXPIRES, so a node that died mid-pass cannot wedge the source forever.
        if let Some(since) = rec.profiling_since {
            if now.saturating_sub(since) < PROFILING_GUARD_SECS {
                pass.skipped += 1;
                continue;
            }
        }
        let job_id = profile_job_id(&rec.source);
        if lb_jobs::load(&node.store, ws, &job_id)
            .await?
            .is_some_and(|j| j.status.is_resumable())
        {
            pass.skipped += 1;
            continue;
        }
        create(
            &node.store,
            ws,
            &lb_jobs::Job::new(&job_id, PROFILE_JOB_KIND, rec.source.clone(), now),
        )
        .await?;
        // Stamp the guard immediately, so the NEXT tick skips this source even if the pass has not
        // started yet. Without this the enqueue is idempotent only via the job record, and a
        // completed-then-restaled job would double up.
        let mut stamped = rec.clone();
        stamped.profiling_since = Some(now);
        put(&node.store, ws, &stamped).await?;
        pass.enqueued += 1;
    }

    // ── 2. drain the queue ──────────────────────────────────────────────────────────────────────
    // `pending` is the indexed (kind, status) lookup — O(pending), not a table walk.
    let launcher = OsLauncher;
    let principal = reactor_principal(ws);
    for mut job in pending(&node.store, ws, PROFILE_JOB_KIND)
        .await?
        .into_iter()
        .take(MAX_PER_TICK)
    {
        let source = job.payload.clone();
        match federation_profile(
            node, &launcher, &principal, ws, &source, None, cfg.bounds, now,
        )
        .await
        {
            Ok(_) => {
                job.status = JobStatus::Done;
                job.ts = now;
                create(&node.store, ws, &job).await?;
                pass.ran += 1;
            }
            Err(e) => {
                tracing::warn!(ws = %ws, source = %source, error = %e, "profile pass failed");
                job.status = JobStatus::Failed;
                job.ts = now;
                create(&node.store, ws, &job).await?;
                // Release the in-flight guard so a later tick can retry; the record keeps its
                // previous profile, which is stale but honest — better than no profile at all.
                if let Some(mut rec) = resolve_profile(&node.store, ws, &source).await? {
                    rec.profiling_since = None;
                    put(&node.store, ws, &rec).await?;
                }
                pass.failed += 1;
            }
        }
    }

    Ok(pass)
}

/// Spawn one lazy ticker driving [`react_to_profiles`] across `workspaces`. The `relay_outbox` /
/// `react_to_reminders` spawn shape: missed ticks are SKIPPED, never queued up, so a node that was
/// suspended does not wake into a backlog of passes.
pub fn spawn_profile_reactors(
    node: Arc<Node>,
    workspaces: Vec<String>,
    period: Duration,
    cfg: ProfileReactorConfig,
) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            for ws in &workspaces {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                match react_to_profiles(&node, ws, now, cfg).await {
                    Ok(p) if p.enqueued > 0 || p.ran > 0 || p.failed > 0 => {
                        tracing::info!(
                            ws = %ws,
                            enqueued = p.enqueued,
                            ran = p.ran,
                            failed = p.failed,
                            "datasource profile pass"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!(ws = %ws, error = %e, "profile reactor pass failed"),
                }
            }
        }
    });
}
