//! The schedule payload's **named window** (relative-time-range scope, build step 7) — a reminder
//! action carries `{"range":{"from":"last-month","to":…,"tz":…}}`, the ONE accepted form. It is
//! **validated at SAVE time** (a bad expression must fail with a human watching, not at 03:00
//! nightly) and **resolved at FIRE time** into concrete `from`/`to` ISO days injected into the
//! payload, so the renderer is handed dates.
//!
//! **The legacy `preset` key is REMOVED, and refused loudly** — at save AND at fire, so a row that
//! predates the removal cannot quietly keep firing. Nothing was in production carrying the seven
//! pre-grammar preset ids, so they were deleted outright rather than aliased (a second vocabulary
//! only drifts). Refusing beats ignoring: a silently-ignored `preset` leaves a reminder that LOOKS
//! configured and mails the fallback window every night.
//!
//! Generic over the action (rule 10): this inspects the payload's `range`/`preset` KEYS, never the
//! target/tool name — an `mcp-tool` action's `args` and an `outbox` action's JSON `payload` get
//! the identical treatment, and a payload without the keys (or a non-JSON outbox payload) passes
//! through untouched.

use lb_reminders::{Action, ReminderError};
use serde_json::Value;

use crate::timerange::{parse_tz, resolve_range};

/// The refusal a removed `preset` key earns — names the dead key AND the replacement, at save and
/// at fire alike (one message, one place, so the two paths cannot drift).
const PRESET_REMOVED: &str =
    "\"preset\" is no longer supported — use range: {from: \"...\"} (a range expression such as \
     last-month, last-7-days, today or now-6h)";

/// Validate the named-window keys of `action`'s payload at SAVE time. `Ok` when neither key is
/// present, when the payload is not a JSON object (opaque, unchanged), or when the `range` parses;
/// a `preset` key or a bad range expression is a loud refusal naming the offender.
pub(super) fn check_action_window(action: &Action) -> Result<(), ReminderError> {
    match action {
        Action::McpTool { args, .. } => check_payload(args),
        Action::Outbox { payload, .. } => match serde_json::from_str::<Value>(payload) {
            Ok(v) => check_payload(&v),
            // A non-JSON payload carries no named window — opaque, as before.
            Err(_) => Ok(()),
        },
        Action::ChannelPost { .. } => Ok(()),
    }
}

/// Resolve `payload`'s `range` (if any) against the fire clock, returning the payload with concrete
/// `from`/`to` ISO days injected (the `range` stays for audit; existing `from`/`to` are replaced —
/// resolution IS the point). `now_secs` is the reminder clock (epoch seconds); tz comes from
/// `range.tz`, else UTC. A payload without a `range` comes back untouched — EXCEPT one carrying the
/// removed `preset` key, which is refused here too (a pre-removal row fails loudly instead of
/// firing a fallback window).
pub(super) fn resolve_payload_window(
    payload: &Value,
    now_secs: u64,
) -> Result<Value, ReminderError> {
    refuse_preset(payload)?;
    let Some(range) = payload.get("range").filter(|r| r.is_object()) else {
        return Ok(payload.clone());
    };
    let (from, to, tz) = range_parts(range)?;
    let resolved = resolve_range(from, to, now_secs as i64 * 1000, tz)
        .map_err(|e| ReminderError::BadInput(format!("range: {e}")))?;
    let mut out = payload.clone();
    if let Some(obj) = out.as_object_mut() {
        obj.insert("from".into(), Value::String(resolved.from_day));
        obj.insert("to".into(), Value::String(resolved.to_day));
    }
    Ok(out)
}

/// Validate one payload object's `range` key — and refuse the removed `preset`.
fn check_payload(payload: &Value) -> Result<(), ReminderError> {
    refuse_preset(payload)?;
    if let Some(range) = payload.get("range").filter(|r| !r.is_null()) {
        if !range.is_object() {
            return Err(ReminderError::BadInput(
                "range must be an object: { from, to?, tz? }".into(),
            ));
        }
        let (from, to, _tz) = range_parts(range)?;
        // Structural validation through the one grammar — fixed clock; the range's own tz was
        // already proven parseable by `range_parts`.
        crate::timerange::validate(from, to)
            .map_err(|e| ReminderError::BadInput(format!("range: {e}")))?;
    }
    Ok(())
}

/// The removed key, refused identically on both paths.
fn refuse_preset(payload: &Value) -> Result<(), ReminderError> {
    match payload.get("preset") {
        Some(p) if !p.is_null() => Err(ReminderError::BadInput(PRESET_REMOVED.into())),
        _ => Ok(()),
    }
}

/// Pull `(from, to?, tz)` out of a `range` object, refusing missing/mis-typed parts loudly.
fn range_parts(range: &Value) -> Result<(&str, Option<&str>, &str), ReminderError> {
    let from = range
        .get("from")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            ReminderError::BadInput("range.from must be a range expression string".into())
        })?;
    let to = range
        .get("to")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty());
    let tz = range.get("tz").and_then(Value::as_str).unwrap_or("");
    // An unknown tz name must refuse at save, not at fire.
    parse_tz(tz).map_err(|e| ReminderError::BadInput(format!("range: {e}")))?;
    Ok((from, to, tz))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 2026-07-29 00:00:00 UTC.
    const JUL_29: u64 = 1_785_283_200;

    fn mcp(args: Value) -> Action {
        Action::McpTool {
            tool: "report.export".into(),
            args,
        }
    }

    #[test]
    fn payloads_without_window_keys_pass_untouched() {
        assert!(check_action_window(&mcp(json!({ "reportId": "energy" }))).is_ok());
        // A non-JSON outbox payload is opaque, as before.
        assert!(check_action_window(&Action::Outbox {
            target: "report".into(),
            action: "render".into(),
            payload: "not json".into(),
        })
        .is_ok());
        let p = json!({ "reportId": "energy" });
        assert_eq!(resolve_payload_window(&p, JUL_29).unwrap(), p);
    }

    #[test]
    fn a_good_range_validates_and_a_bad_one_refuses_naming_the_token() {
        assert!(check_action_window(&mcp(json!({ "range": { "from": "last-month" } }))).is_ok());
        assert!(check_action_window(&mcp(
            json!({ "range": { "from": "now-7d/d", "to": "now/d", "tz": "Australia/Sydney" } })
        ))
        .is_ok());

        let e = check_action_window(&mcp(json!({ "range": { "from": "last-fortnight" } })))
            .unwrap_err()
            .to_string();
        assert!(e.contains("last-fortnight"), "names the token: {e}");
        // A range token with a `to` is the shape error the grammar refuses.
        assert!(check_action_window(&mcp(
            json!({ "range": { "from": "this-month", "to": "now" } })
        ))
        .is_err());
        // An unknown tz refuses at save, not at 03:00.
        let e = check_action_window(&mcp(
            json!({ "range": { "from": "yesterday", "tz": "Mars/Olympus" } }),
        ))
        .unwrap_err()
        .to_string();
        assert!(e.contains("Mars/Olympus"), "{e}");
        // The OUTBOX form (the shipped report schedule) is judged identically.
        assert!(check_action_window(&Action::Outbox {
            target: "report".into(),
            action: "render".into(),
            payload: json!({ "reportId": "energy", "range": { "from": "nope" } }).to_string(),
        })
        .is_err());
    }

    /// The removed `preset` key is refused LOUDLY — at save and at fire, in both carriers. A silent
    /// ignore would leave a reminder that looks configured and mails a fallback window nightly.
    #[test]
    fn the_removed_preset_key_is_refused_at_save_and_at_fire() {
        for action in [
            mcp(json!({ "reportId": "energy", "preset": "last-7-days" })),
            Action::Outbox {
                target: "report".into(),
                action: "render".into(),
                payload: json!({ "reportId": "energy", "preset": "last-7-days" }).to_string(),
            },
        ] {
            let e = check_action_window(&action).unwrap_err().to_string();
            assert!(e.contains("preset"), "names the dead key: {e}");
            assert!(e.contains("range:"), "names the replacement: {e}");
        }
        // FIRE time: a row that predates the removal fails loudly rather than firing a fallback.
        let e = resolve_payload_window(&json!({ "preset": "last-7-days" }), JUL_29)
            .unwrap_err()
            .to_string();
        assert!(e.contains("preset") && e.contains("range:"), "{e}");
    }

    /// Fire-time resolution injects concrete ISO days and re-resolves per firing — the entire point
    /// of storing a name instead of an epoch.
    #[test]
    fn fire_time_resolution_injects_concrete_dates() {
        let p = json!({ "reportId": "energy", "range": { "from": "last-month" } });
        let out = resolve_payload_window(&p, JUL_29).unwrap();
        assert_eq!(out["from"], "2026-06-01");
        assert_eq!(out["to"], "2026-07-01");
        assert_eq!(
            out["range"]["from"], "last-month",
            "the name stays for audit"
        );

        // A month later the SAME stored payload names the next window.
        let later = resolve_payload_window(&p, JUL_29 + 31 * 86_400).unwrap();
        assert_eq!(later["from"], "2026-07-01");
        assert_eq!(later["to"], "2026-08-01");
    }
}
