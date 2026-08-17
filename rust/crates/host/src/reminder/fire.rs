//! `fire_reminder` — dispatch ONE firing's action under the reminder's **stored principal**, re-
//! resolved at fire time (reminders scope "principal capture at fire time"). This is the job's
//! body: the reactor enqueues a `kind="reminder-fire"` lb-jobs job, and this is what that job does.
//!
//! The security model in one line: the firing re-checks the action's OWN capability under the
//! stored principal's CURRENTLY-resolved caps. A grant revoked after create turns the firing into a
//! logged deny — never a privilege-escalation backdoor (the action never runs). Caps are re-resolved
//! from the durable grant store ([`crate::authz::resolve_caps`]), so a revoke takes effect at the
//! next fire, not just the next token re-mint.
//!
//! Each action kind dispatches against its REAL seam (no stubs):
//!   - **ChannelPost** → the channel service (`crate::channel::post`), which re-checks
//!     `bus:chan/{channel}:pub` and writes a durable `lb_inbox::Item`.
//!   - **McpTool** → re-enters the host `call_tool` chokepoint, which re-checks `mcp:{tool}:call`
//!     (authoritative validation at fire time — tool schemas evolve between create and fire).
//!   - **Outbox** → `enqueue_outbox`, which re-checks `mcp:outbox.enqueue:call` and stages a
//!     pending `Effect` (the relay owns delivery).

use std::sync::Arc;

use lb_auth::Principal;
use lb_reminders::{Action, Reminder, ReminderError};
use lb_store::Store;
use serde_json::Value;

use crate::authz::resolve_caps_live as resolve_caps;
use crate::boot::Node;

/// The deterministic lb-jobs job id for one firing of `reminder_id` at `scheduled_ts`. Stable, so a
/// re-scan addresses the same job record and the existence check makes the reactor idempotent (one
/// scheduled instant → one job → one effect).
pub fn fire_job_id(reminder_id: &str, scheduled_ts: u64) -> String {
    format!("reminder-fire:{reminder_id}:{scheduled_ts}")
}

/// The lb-jobs job kind for a reminder firing. The reactor tags every enqueued firing job with it.
pub const FIRE_KIND: &str = "reminder-fire";

/// The prefix a user principal's `sub` carries and the durable grant store does NOT.
const USER_PREFIX: &str = "user:";

/// Dispatch the action for `reminder` (whose `next_attempt_ts` is `scheduled_ts`) at logical time
/// `now`, under the reminder's stored principal (caps re-resolved from the grant store). A denial
/// is returned as [`ReminderError::Denied`] — the reactor logs it, counts it, and advances the
/// reminder to its next slot (the stable job id keeps a re-scan from double-firing this instant; see
/// `react.rs` for why NOT advancing killed the schedule outright).
pub async fn fire_reminder(
    node: &Arc<Node>,
    ws: &str,
    reminder: &Reminder,
    scheduled_ts: u64,
    now: u64,
) -> Result<(), ReminderError> {
    let principal = resolve_fire_principal(&node.store, ws, &reminder.principal_sub).await?;
    match &reminder.action {
        Action::ChannelPost { channel, body } => {
            fire_channel_post(
                node,
                &principal,
                ws,
                &reminder.id,
                scheduled_ts,
                channel,
                body,
                now,
            )
            .await
        }
        Action::McpTool { tool, args } => {
            fire_mcp_tool(node, &principal, ws, tool, args, now).await
        }
        Action::Outbox {
            target,
            action,
            payload,
        } => {
            fire_outbox(
                &node.store,
                &principal,
                ws,
                &reminder.id,
                scheduled_ts,
                target,
                action,
                payload,
                now,
            )
            .await
        }
    }
}

/// Re-resolve the stored principal's CURRENT caps from the durable grant store, then rebuild a
/// principal the check path accepts. This is the load-bearing re-check: a revoke between create and
/// fire drops the cap here, so the action's own gate then denies. Node-local co-trust (same posture
/// as `Principal::routed`): the reminder record is workspace-authoritative state.
async fn resolve_fire_principal(
    store: &Store,
    ws: &str,
    sub: &str,
) -> Result<Principal, ReminderError> {
    // Grants are stored under the BARE user name — `resolve_caps` re-wraps it as `Subject::User`,
    // so handing it the `user:`-prefixed sub looks up `Subject::User("user:test")` and finds NOTHING.
    // The session mint has always stripped it (`role/gateway/src/session/mint_session.rs`); this path
    // did not, and the consequence was total: EVERY reminder authored by a real user resolved ZERO
    // caps at fire time, so every action kind was denied, for every reminder, always. It presented as
    // `denied=1` in the reactor's pass line with no reason, and (see `react.rs`) the reminder was then
    // left un-advanced behind its own idempotency marker, so it never even retried.
    let bare = sub.strip_prefix(USER_PREFIX).unwrap_or(sub);
    let caps = resolve_caps(store, ws, bare).await?;
    // The principal keeps the PREFIXED sub: that is the identity every downstream gate, audit row and
    // `by` field records. Only the grant lookup wants the bare handle.
    Ok(Principal::routed(sub, ws, caps))
}

// Argument count is the explicit dependency list; bundling it into a struct would be a refactor.
#[allow(clippy::too_many_arguments)]
async fn fire_channel_post(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    reminder_id: &str,
    scheduled_ts: u64,
    channel: &str,
    body: &str,
    now: u64,
) -> Result<(), ReminderError> {
    // Re-check the action's own cap (`bus:chan/{channel}:pub`) under the stored principal — the
    // security model. Uses the channel service's authorize chokepoint, the same gate `post` runs,
    // so a revoked grant denies here (no escalation). The durable `lb_inbox::Item` is then written
    // directly (the scope's seam: "channel post → real lb_inbox item in the channel"); the live bus
    // echo is best-effort motion (the record is the truth, §3.3). This is deliberately decoupled
    // from `channel::post` so a reminder firing rides only the stable record + cap seams.
    crate::channel::authorize_channel(principal, ws, channel, lb_caps::Action::Pub)
        .map_err(|_| ReminderError::Denied)?;

    let author = principal.sub().to_string();
    let item = lb_inbox::Item::new(
        // Stable item id from (reminder, scheduled_ts): a re-delivery upserts the same row.
        fire_job_id(reminder_id, scheduled_ts),
        channel,
        author,
        body.to_string(),
        now,
    );
    lb_inbox::record(&node.store, ws, &item).await?;

    // Register-on-post keeps the channel listable (create-on-first-post). Best-effort: a registry
    // hiccup must never fail the firing (the durable item is the source of truth).
    let _ =
        crate::channel_registry::register_on_post(&node.store, ws, channel, principal.sub(), now)
            .await;

    // MOTION: best-effort live echo on the channel bus subject. A bus failure is non-fatal to the
    // durable firing (the record already landed).
    if let Ok(payload) = serde_json::to_vec(&item) {
        let _ = lb_bus::publish(
            &node.bus,
            ws,
            &crate::channel::msg_key_for(channel, &item.id),
            &payload,
        )
        .await;
    }
    Ok(())
}

async fn fire_mcp_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    tool: &str,
    args: &Value,
    now: u64,
) -> Result<(), ReminderError> {
    // Authoritative validation at fire time: re-enter the one call_tool chokepoint, which re-checks
    // workspace-first + `mcp:{tool}:call` under the stored principal. A denied/missing tool is a
    // logged deny here (no effect); a tool error is surfaced.
    //
    // A named window (`range`) in the args resolves against the FIRE clock into concrete `from`/`to`
    // ISO days first (relative-time-range scope, step 7) — the tool is handed dates, not a name.
    let input = if args.is_null() {
        "{}".to_string()
    } else {
        super::range::resolve_payload_window(args, now)?.to_string()
    };
    crate::tool_call::call_tool(node, principal, ws, tool, &input)
        .await
        .map(|_| ())
        .map_err(|_| ReminderError::Denied)
}

// Argument count is the explicit dependency list; bundling it into a struct would be a refactor.
#[allow(clippy::too_many_arguments)]
async fn fire_outbox(
    store: &Store,
    principal: &Principal,
    ws: &str,
    reminder_id: &str,
    scheduled_ts: u64,
    target: &str,
    action: &str,
    payload: &str,
    now: u64,
) -> Result<(), ReminderError> {
    // The effect id is derived from (reminder, scheduled_ts) — stable, so a re-enqueue is a no-op.
    let effect_id = fire_job_id(reminder_id, scheduled_ts);
    // A JSON payload carrying a named window (`range`) resolves against the FIRE clock into
    // concrete `from`/`to` ISO days before the effect is staged — the target is handed dates
    // (relative-time-range scope, step 7). A non-JSON payload is opaque and rides unchanged.
    // A `preset` key is matched too, NOT to resolve it but to REFUSE it: the legacy preset
    // vocabulary was removed, and a row that predates the removal must fail loudly here rather
    // than quietly mail a fallback window every night.
    let payload = match serde_json::from_str::<Value>(payload) {
        Ok(v) if v.get("range").is_some() || v.get("preset").is_some() => {
            super::range::resolve_payload_window(&v, now)?.to_string()
        }
        _ => payload.to_string(),
    };
    crate::outbox::enqueue_outbox(
        store, principal, ws, &effect_id, target, action, &payload, now,
    )
    .await
    .map_err(|e| {
        // The caller only ever sees `Denied` — deliberately, so a firing cannot probe the caps wall.
        // But the reactor then reports `denied=1` and LEAVES the reminder scheduled, and the
        // underlying reason was thrown away here, so an operator watching a schedule that silently
        // stops firing has nothing at all to go on. `enqueue_outbox` returns `Denied` for the cap
        // and a store error for anything else; those want completely different fixes and must not
        // look identical in the log.
        tracing::warn!(
            reminder = %reminder_id, ws, target, action, effect = %effect_id, error = %e,
            "reminder firing could not stage its outbox effect"
        );
        ReminderError::Denied
    })
}
