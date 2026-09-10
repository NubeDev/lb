//! The reminder lint — what stands between a pack author's typo and a schedule that applies wrong,
//! or applies right and then silently replays one answer forever.
//!
//! `manifest_reminder` holds `action_kind` as a `String` and every per-kind field as an `Option`
//! (it is a dependency-free mirror of the verb args — see that module for why), so nothing in the
//! type system rejects `action_kind: mcp_tool` or an `mcp-tool` with no `tool:`. The apply-side
//! conversion cannot reject those without failing the whole pack opaquely. So they are caught HERE,
//! at validate time, where the author is still looking and the message can name the closed set.
//! Errors gate the apply (`validate.rs` module doc).

use crate::manifest_reminder::{PackReminder, ACTION_KINDS};
use crate::validate::Finding;

/// Lint every pack-declared reminder: the action kind, the fields that kind requires, the cron's
/// field count, and the `{{fire_ts}}` replay trap.
pub fn lint(reminders: &[PackReminder]) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in reminders {
        // ERROR — an unknown action kind. The manifest is a MIRROR (the kind is a `String`), so this
        // lint is the only thing between a typo and an apply that fails with no line to point at.
        if !ACTION_KINDS.contains(&r.action_kind.as_str()) {
            out.push(Finding {
                error: true,
                message: format!(
                    "reminder '{}': unknown action_kind '{}' — expected one of {}",
                    r.id,
                    r.action_kind,
                    ACTION_KINDS.join(", ")
                ),
            });
            // The per-kind field checks below are meaningless once the kind itself is wrong.
            continue;
        }

        // ERROR — the fields that kind actually needs. `reminder.create`'s own best-effort check
        // rejects an EMPTY tool/channel/target, but an ABSENT one never reaches it: the mirror would
        // convert `None` into an empty string and the failure would surface as a create error with
        // no manifest line attached. Named here instead.
        let required: &[(&str, bool)] = match r.action_kind.as_str() {
            "channel-post" => &[("channel", r.channel.is_some()), ("body", r.body.is_some())],
            "mcp-tool" => &[("tool", r.tool.is_some())],
            _ => &[
                ("target", r.target.is_some()),
                ("action", r.action.is_some()),
            ],
        };
        for (field, present) in required {
            if !present {
                out.push(Finding {
                    error: true,
                    message: format!(
                        "reminder '{}': action_kind '{}' needs a '{field}'",
                        r.id, r.action_kind
                    ),
                });
            }
        }

        // ERROR — the cron's field count. The authoritative parse is `is_valid` at apply time; this
        // catches the overwhelmingly common shape error (a 6-field quartz cron, or an empty string)
        // with the author still looking, rather than as an apply-time `BadCron`.
        let fields = r.schedule.split_whitespace().count();
        if fields != 5 {
            out.push(Finding {
                error: true,
                message: format!(
                    "reminder '{}': schedule '{}' has {fields} fields — a 5-field cron is expected \
                     (min hour dom mon dow)",
                    r.id, r.schedule
                ),
            });
        }

        // WARNING — the replay trap. Reminder args are stored VERBATIM, and a verb idempotent on a
        // caller-supplied id (`agent.invoke` on `job_id`) returns its FIRST answer on every firing
        // when that id never changes: a frozen agent that looks perfectly alive (runs climbing, no
        // error, a plausible answer). `{{fire_ts}}` is the fix. A warning, not an error, because
        // the manifest cannot know which verbs are idempotent on which field — this is the one
        // shape where the intent is unmistakable.
        if let Some(job_id) = r
            .args
            .as_ref()
            .and_then(|a| a.get("job_id"))
            .and_then(|v| v.as_str())
        {
            if !job_id.contains("{{fire_ts}}") {
                out.push(Finding {
                    error: false,
                    message: format!(
                        "reminder '{}': job_id '{job_id}' is the same on every firing — a verb that \
                         is idempotent on it will replay the first run's answer forever. Vary it \
                         with the {{{{fire_ts}}}} placeholder.",
                        r.id
                    ),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn base() -> PackReminder {
        PackReminder {
            id: "r1".into(),
            schedule: "0 6 * * *".into(),
            action_kind: "mcp-tool".into(),
            channel: None,
            body: None,
            tool: Some("rules.run".into()),
            args: None,
            target: None,
            action: None,
            payload: None,
            max_runs: None,
        }
    }

    #[test]
    fn a_well_formed_reminder_lints_clean() {
        assert!(lint(&[base()]).is_empty());
    }

    #[test]
    fn an_unknown_action_kind_names_the_closed_set() {
        let r = PackReminder {
            action_kind: "mcp_tool".into(),
            ..base()
        };
        let f = lint(&[r]);
        assert_eq!(
            f.len(),
            1,
            "the kind is wrong, so per-kind checks are skipped"
        );
        assert!(f[0].error);
        assert!(
            f[0].message.contains("mcp-tool"),
            "names the set: {}",
            f[0].message
        );
    }

    #[test]
    fn a_kind_missing_its_required_field_is_an_error() {
        let r = PackReminder {
            tool: None,
            ..base()
        };
        let f = lint(&[r]);
        assert!(f
            .iter()
            .any(|f| f.error && f.message.contains("needs a 'tool'")));
    }

    #[test]
    fn a_six_field_cron_is_caught_before_apply() {
        let r = PackReminder {
            schedule: "0 0 6 * * *".into(),
            ..base()
        };
        let f = lint(&[r]);
        assert!(f.iter().any(|f| f.error && f.message.contains("6 fields")));
    }

    /// The replay trap: a static `job_id` warns, and the message names the fix.
    #[test]
    fn a_static_job_id_warns_about_the_replay() {
        let r = PackReminder {
            tool: Some("agent.invoke".into()),
            args: Some(json!({ "job_id": "daily-review", "goal": "review" })),
            ..base()
        };
        let f = lint(&[r]);
        assert_eq!(f.len(), 1);
        assert!(
            !f[0].error,
            "a warning — the manifest cannot know which verbs are idempotent"
        );
        assert!(
            f[0].message.contains("{{fire_ts}}"),
            "names the fix: {}",
            f[0].message
        );
    }

    #[test]
    fn a_varying_job_id_is_clean() {
        let r = PackReminder {
            tool: Some("agent.invoke".into()),
            args: Some(json!({ "job_id": "daily-review-{{fire_ts}}" })),
            ..base()
        };
        assert!(lint(&[r]).is_empty());
    }

    /// Args without a `job_id` at all are none of this lint's business.
    #[test]
    fn args_without_a_job_id_are_not_linted() {
        let r = PackReminder {
            args: Some(json!({ "rule_id": "fdd-sensor-flatline" })),
            ..base()
        };
        assert!(lint(&[r]).is_empty());
    }
}
