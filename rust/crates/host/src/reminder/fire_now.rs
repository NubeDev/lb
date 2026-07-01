//! `reminder_fire` — the gated, idempotent **run-now** verb (channel rich responses + reminders-tenant
//! scope). Fires ONE firing of a reminder immediately, on demand (a "run now" button), REUSING the
//! shipped internal fire path ([`fire_reminder`]) — it does NOT duplicate dispatch.
//!
//! Idempotency — **double-fire in the same instant → one action** — is enforced exactly like the
//! reactor scan: the deterministic per-firing job id ([`fire_job_id`]) is the dedup marker. A run-now
//! writes the marker BEFORE dispatch (crash-safe), so a double-click in the same logical `now` finds
//! the job and no-ops.
//!
//! The instant choice: a MANUAL fire uses `scheduled_ts = now` (NOT the reminder's `next_attempt_ts`).
//! Using `next_attempt_ts` would risk colliding with a legitimate scheduled fire's job id (and being
//! wrongly skipped, or wrongly advancing the schedule). Keying on `now` makes the manual firing its
//! own instant: two run-now clicks in the same logical `now` dedupe (idempotent), while a scheduled
//! fire uses its own `next_attempt_ts` — the two never collide. Run-now does NOT advance the reminder
//! (it is a manual extra firing, not a schedule advance) — it only writes the job marker and fires.

use std::sync::Arc;

use lb_auth::Principal;
use lb_jobs::{create, load, Job};
use lb_reminders::ReminderError;
use serde_json::{json, Value};

use super::authorize::authorize_reminder;
use super::fire::{fire_job_id, fire_reminder, FIRE_KIND};
use super::get::reminder_get;
use crate::boot::Node;

/// Fire reminder `id` NOW, once, in workspace `ws` as `principal`. Gate-first
/// (`mcp:reminder.fire:call`, workspace-first — the ws is the token's, never args). Loads the reminder
/// (opaque `NotFound` if absent), then — keyed on the deterministic job id for `(id, now)` — writes
/// the firing job BEFORE dispatch and calls the shipped [`fire_reminder`]. A second run-now at the same
/// logical `now` finds the existing job and returns `{"fired": false}` (no double-fire). Returns
/// `{"fired": true, "scheduled_ts": <now>}` on a fresh firing.
pub async fn reminder_fire(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<Value, ReminderError> {
    // Gate FIRST (opaque `Denied`). `authorize_reminder` builds `mcp:reminder.fire:call` from the verb.
    authorize_reminder(principal, ws, "fire")?;

    // Load under the caller's own read (the ws is the token's — a leaked ws-A id can't be fired from
    // ws-B, the store namespace is ws-B's, so it reads as absent). `reminder_get` re-checks the get
    // gate, but the caller reaching a run-now already holds fire; a missing get grant is a clean deny.
    let reminder = reminder_get(&node.store, principal, ws, id)
        .await?
        .ok_or(ReminderError::NotFound)?;

    // Manual instant = `now` (see module docs: never `next_attempt_ts`, to avoid colliding with a
    // scheduled fire's job id).
    let scheduled_ts = now;
    let job_id = fire_job_id(id, scheduled_ts);

    // Idempotency: a job already exists for this (reminder, instant) → no-op (no double-fire).
    if load(&node.store, ws, &job_id).await?.is_some() {
        return Ok(json!({ "fired": false }));
    }

    // Record the durable firing job BEFORE dispatch, so a crash mid-fire leaves an idempotent marker
    // (a re-click in the same instant finds the job and skips). Same crash-safe order as the reactor.
    let payload =
        json!({ "reminder_id": id, "scheduled_ts": scheduled_ts, "run_now": true }).to_string();
    create(&node.store, ws, &Job::new(&job_id, FIRE_KIND, payload, now)).await?;

    // Dispatch through the SHIPPED internal fire path (reused, not duplicated). Run-now does NOT
    // advance the reminder — it is a manual extra firing, not a schedule step.
    //
    // `Box::pin` breaks an async-recursion cycle: an `mcp-tool` action re-enters `call_tool` →
    // dispatch → `call_reminder_tool` → back HERE, so the future would be infinitely sized without
    // indirection (E0733). Boxing this one edge cuts the cycle (the reactor's path has no such cycle
    // because it is not reached from the `reminder.*` dispatch).
    Box::pin(fire_reminder(node, ws, &reminder, scheduled_ts, now)).await?;
    Ok(json!({ "fired": true, "scheduled_ts": scheduled_ts }))
}
