//! **Scheduling and cancelling the nudge ladder** — three one-shot reminders per ask (case-plane
//! scope, wave 2).
//!
//! Each instant from [`super::request_window::nudge_instants`] becomes one durable
//! [`lb_reminders::Reminder`] that fires `case.request.nudge` under the **sender's** principal. Two
//! consequences worth stating, because both are the point:
//!
//! - **The nudge re-checks the sender's caps at fire time**, like every reminder does. Revoke the
//!   sender's `mcp:case.request.send:call` and the ladder stops — the schedule cannot outlive the
//!   authority that created it (`green-while-broken-reactor-tests.md`: a nudge test that never
//!   turns that cap off is green over a broken wall).
//! - **The reminders are written through the crate verbs, not the `reminder.*` MCP surface.** A
//!   member sending an ask must not need `mcp:reminder.create:call` — the schedule is part of what
//!   `case.request.send` *is*, not a second thing the caller is separately entitled to. The host
//!   service owns its own schedule, exactly as the case reactors own their own writes.
//!
//! **A one-shot at an instant, spelled as cron.** `lb-reminders` stores a 5-field cron and computes
//! `next_attempt_ts` from it; there is no "fire once at T" action shape. So the reminder carries the
//! minute-pinned cron for the target instant (`M H D Mo *`), `max_runs: 1`, and — the load-bearing
//! line — `next_attempt_ts` set **explicitly** to the target second afterwards. Without that last
//! step a short test window would floor to a minute already past and croner would schedule the same
//! minute *next year*: the nudge would look scheduled and never fire.

use lb_reminders::{save, Action, Reminder};
use lb_store::Store;
use serde_json::json;

use super::error::CaseSvcError;

/// The three ladder stages, in order — also the suffix of each reminder's id.
pub(super) const NUDGE_STAGES: [&str; 3] = ["n50", "n80", "breach"];

/// The reminder id for one stage of one request. No `:` — a reminder id is a store record-id
/// segment (the trap an asset id already documents).
pub(super) fn nudge_reminder_id(request_id: &str, stage: &str) -> String {
    format!("case-nudge-{request_id}-{stage}")
}

/// Schedule the ladder for `request_id`: one reminder per instant, firing `case.request.nudge`
/// under `sender_sub`.
///
/// Best-effort per stage: a stage that cannot be scheduled is logged and the others still land. A
/// failed nudge must never fail the ask itself — the link is already in the outbox, and losing the
/// reminder costs a follow-up, while losing the send costs the job.
pub(super) async fn schedule_nudges(
    store: &Store,
    ws: &str,
    request_id: &str,
    sender_sub: &str,
    instants_ms: [u64; 3],
    now_ms: u64,
) -> Result<(), CaseSvcError> {
    for (stage, at_ms) in NUDGE_STAGES.iter().zip(instants_ms.iter()) {
        if *at_ms <= now_ms {
            // A window so short that a stage is already in the past: skip it rather than schedule a
            // reminder that fires immediately. The breach stage of a 1-minute test window is the
            // realistic case, and firing "you are late" in the same second as the ask is noise.
            continue;
        }
        let at_secs = at_ms / 1000;
        // `ts` is the SCHEDULED instant, carried in the args. The reminder reactor calls the tool
        // with no clock of its own, so without this the rung would act at the host's wall clock —
        // and a breach rung firing late would then judge the window against "now" instead of
        // against the deadline it was scheduled for. The logical time of a rung IS its instant.
        let action = Action::McpTool {
            tool: super::NUDGE_TOOL.to_string(),
            args: json!({ "id": request_id, "stage": stage, "ts": at_ms }),
        };
        let mut reminder = Reminder::new(
            nudge_reminder_id(request_id, stage),
            cron_at(at_secs),
            Some(1),
            action,
            sender_sub,
            now_ms / 1000,
        );
        // See the module note: this line, not the cron string, is what makes the instant exact.
        reminder.next_attempt_ts = at_secs;
        if let Err(e) = save(store, ws, &reminder).await {
            tracing::warn!(
                request = %request_id, stage = %stage, error = %e,
                "case request: could not schedule a nudge — the ask stands, the follow-up will not fire"
            );
        }
    }
    Ok(())
}

/// Cancel every outstanding nudge for `request_id` — on a reply, on a withdrawal, on expiry.
///
/// Tombstones through the crate's own save (the `reminder_delete` shape) rather than deleting the
/// row, so the history of "we were going to chase them twice" survives the answer. Idempotent: a
/// stage that never existed, or is already tombstoned, is a no-op.
pub(super) async fn cancel_nudges(store: &Store, ws: &str, request_id: &str) {
    for stage in NUDGE_STAGES {
        let id = nudge_reminder_id(request_id, stage);
        match lb_reminders::load(store, ws, &id).await {
            Ok(Some(mut reminder)) if !reminder.deleted => {
                reminder.deleted = true;
                reminder.enabled = false;
                if let Err(e) = save(store, ws, &reminder).await {
                    tracing::warn!(
                        request = %request_id, stage = %stage, error = %e,
                        "case request: could not cancel a nudge — the party may be chased after answering"
                    );
                }
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(
                request = %request_id, stage = %stage, error = %e,
                "case request: could not read a nudge to cancel it"
            ),
        }
    }
}

/// The 5-field cron matching exactly the minute `at_secs` falls in, in UTC.
fn cron_at(at_secs: u64) -> String {
    use chrono::{DateTime, Datelike, Timelike, Utc};
    let dt = DateTime::<Utc>::from_timestamp(at_secs as i64, 0).unwrap_or_default();
    format!(
        "{} {} {} {} *",
        dt.minute(),
        dt.hour(),
        dt.day(),
        dt.month()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cron_pins_the_minute_the_instant_falls_in() {
        // 2026-09-10T04:05:00Z
        let secs = chrono::DateTime::parse_from_rfc3339("2026-09-10T04:05:30Z")
            .unwrap()
            .timestamp() as u64;
        assert_eq!(cron_at(secs), "5 4 10 9 *");
        assert!(
            lb_reminders::is_valid(&cron_at(secs)),
            "the generated cron must parse — an invalid one silently kills the ladder"
        );
    }

    #[test]
    fn a_reminder_id_is_one_record_id_segment() {
        let id = nudge_reminder_id("01J8ZQ", "n50");
        assert!(!id.contains(':'), "{id}");
        assert!(!id.contains('.'), "{id}");
    }
}
