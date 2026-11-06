# Global schedules in the rubix-ce shape (scope)

Status: building (2026-11-04). Session: `../../sessions/flows/schedule-rubix-ce-shape-session.md`.

## The ask

The global schedule record (`schedule.*`) is replaced by the rubix-ce schedule document, so a
downstream sync can push one record to many device schedules. The full design, decisions D1 to D14
and the order of work across repos, lives with the downstream product:
`NubeIO/rust-schedule-management` `docs/superpowers/specs/2026-11-04-node-schedule-sync-design.md`.
This scope covers only what lb owns (D2 to D7).

## What lb ships

- The record carries the rubix-ce document under `schedule` (same key names), plus `description`,
  `color`, the four payload fields, and per dated entry `kind`, `label`, `yearly`.
- The old `weekly` / `exceptions` shape and its engine are removed. A stored row whose `schedule`
  document is missing or does not decode is refused: `get`/`evaluate` answer `schedule_old_shape`
  ("does not read as a schedule (...)" when the document is present but unreadable), `list` names it
  under `unreadable` with its `rev`, and a flow `schedule` node keeps its last state without firing.
  No conversion in lb; such rows are saved over in place.
- `lb-schedules`: validation, evaluation by the box's rules, next change, zones. Pure.
- `schedule.save` checks `rev` in one transaction (`lb_store::write_if_rev`); no `rev` is a create
  that is refused when the id exists; the old top-level fields are refused.
- `schedule.evaluate` adds `entry_name` and `value`. `next_transition` is an RFC3339 UTC instant,
  converted through the record's zone (DST-correct), and `null` when `enabled` is false.

Evaluation rules worth stating:

- Overlaps: the entry that started earliest gives the name and value. On the same minute a weekly
  entry beats an event; otherwise keys sort naturally (`w2` before `w10`).
- 29 February: a yearly entry that starts on 29 Feb is skipped in years without one; a yearly end on
  a missing 29 Feb becomes 1 March 00:00.
- Accepted edge: on the night DST ends, a dated boundary inside the repeated hour can count twice.

This is a breaking change to every `schedule.*` caller.

## How it fits the core

- Capabilities: no new caps. `store:schedule:read|write` and `mcp:schedule.*:call` as before.
  Deny tests: `rust/crates/host/tests/schedule/read_rev.rs`, `save_rev.rs`.
- Tenancy: every read and write is ws-scoped; isolation tests in the same files and in
  `rust/crates/store/tests/write_if_rev_test.rs`.
- Rule 10: nothing names an extension; the sync extension is a plain caller of `schedule.*`.

## Open questions

- None for lb. Decisions taken while building are listed in the session doc.
