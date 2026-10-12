# Reminders — a schedule fires at its own zone's clock time (session)

- Date: 2026-10-12
- Scope: ../../scope/reminders/reminders-scope.md (API shape, the `tz` paragraph)
- Status: built + green; lands on the next `node-v*` tag

## Why

Every reminder cron was read in UTC (`next_after` handed croner a `DateTime<Utc>`), so a product
host's "Daily at 9:00 AM" fired at 09:00 UTC — 8 PM in Sydney — and the host could only say so in a
footnote. The report window already resolved in the job's own zone (`range.tz`, relative-time-range
scope); the firing instant did not.

## What changed

- `lb-reminders`: `Reminder.tz: String` (`#[serde(default, skip_serializing_if = empty)]`, so an
  old row reads back unchanged and a UTC row writes no new key). `next_after_in(schedule, after,
  tz)` converts the logical instant into the zone (`chrono-tz`) before croner's generic
  `find_next_occurrence`, then back to seconds; `next_after` is the UTC form and still serves the
  flow cron and the rule preview. `is_valid_tz` for the save-time check; an unknown name is
  `BadInput` naming it.
- `lb-host`: `reminder_create` takes `tz` (one more positional, after `schedule`); `ReminderPatch.tz`
  on update, and a zone change re-anchors like a reschedule; the reactor's `advance` and
  `reschedule` read the cron in the record's zone, so 9 AM stays 9 AM across a DST change. The MCP
  verbs accept `tz` and the wire row carries it; the `reminder.create` palette descriptor declares
  it. Pack-seeded schedules pass `""` (UTC, as before).
- Tests: four zone cases in `reminders/tests/next_after_test.rs` (Sydney hour and weekday, the
  empty-zone identity, the refusal), `a_schedule_fires_at_its_own_zones_clock_time` in
  `host/tests/reminders_mcp_test.rs` (create with a zone, update back to UTC, unknown zone is
  `BadInput`). Every positional test caller gained the argument.

## Not done

- A pack manifest cannot state a zone yet (`PackReminder` has no `tz`); packs stay UTC.
- The flow cron (`flows/react_cron.rs`) and the rule schedule preview still run in UTC.
