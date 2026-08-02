//! `federation.profile_refresh {source}` → `{job_id}` (datasource-profile scope) — force a rebuild
//! of a source's discovery profile, WITHOUT blocking the caller.
//!
//! It **enqueues** an `lb-jobs` job (kind [`PROFILE_JOB_KIND`]) rather than running the pass inline,
//! the `docs.reindex` shape: a profiling pass against a slow or huge source can take minutes, and a
//! verb that blocks on it turns an admin's "please refresh this" into a hung request. The reactor
//! drains the queue.
//!
//! **This verb gets its OWN capability** — `mcp:federation.profile_refresh:call` — unlike
//! `profile`/`profile_get`, which ride the existing read cap. The distinction is cost, not secrecy:
//! reading a profile reveals nothing a `SELECT` could not, but *refreshing* spends real work on
//! someone else's database on demand, so it is separately grantable and separately revokable.
//!
//! Enqueue is IDEMPOTENT. The job id is derived from the source, so a burst of refresh calls
//! collapses onto one durable job; and a job that is still resumable is returned as-is rather than
//! restarted.

use lb_auth::Principal;
use lb_jobs::{create, load, Job};
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::FederationError;
use super::record::resolve;
use crate::boot::Node;

/// The `lb-jobs` kind the profiling queue rides. One kind, drained by `react_to_profiles`.
pub const PROFILE_JOB_KIND: &str = "datasource_profile";

/// The durable job id for profiling `source`. Deterministic (not time-derived) so concurrent
/// enqueues — a manual refresh racing the reactor's tick racing a register-time enqueue — all
/// address the SAME job record instead of creating three passes over one database.
pub fn profile_job_id(source: &str) -> String {
    format!("datasource-profile:{source}")
}

/// Enqueue a profiling pass for `source` in `ws` as `caller`; returns the durable job id.
pub async fn federation_profile_refresh(
    node: &Node,
    caller: &Principal,
    ws: &str,
    source: &str,
    ts: u64,
) -> Result<Value, FederationError> {
    // Its own cap: refreshing SPENDS external-DB work on demand (reading the result does not).
    authorize(caller, ws, "federation.profile_refresh")?;

    // Resolve the alias in this workspace — un-spoofable, and it refuses to enqueue work for a
    // source that does not exist here.
    resolve(&node.store, ws, source)
        .await?
        .ok_or(FederationError::NotFound)?;

    let job_id = profile_job_id(source);
    match load(&node.store, ws, &job_id).await? {
        // Already queued/running: hand back the same id. Re-enqueueing would either duplicate the
        // pass or clobber a running job's checkpoint.
        Some(j) if j.status.is_resumable() => Ok(json!({ "job_id": job_id, "enqueued": false })),
        _ => {
            let job = Job::new(&job_id, PROFILE_JOB_KIND, source.to_string(), ts);
            create(&node.store, ws, &job).await?;
            Ok(json!({ "job_id": job_id, "enqueued": true }))
        }
    }
}

/// The palette/agent descriptor for `federation.profile_refresh`.
pub fn profile_refresh_descriptor() -> lb_mcp::ToolDescriptor {
    lb_mcp::ToolDescriptor {
        emits_external: false,
        name: "federation.profile_refresh".to_string(),
        title: "Queue a rebuild of a datasource's discovery profile".to_string(),
        group: "federation".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source": { "type": "string", "x-lb": { "entity": "datasource" } }
            },
            "required": ["source"]
        })),
        result: Some(json!({
            "v": 2,
            "view": "jsonview",
            "source": { "tool": "federation.profile_get", "args": {} },
            "options": { "collapsed": true },
            "tools": ["federation.profile_get"]
        })),
    }
}
