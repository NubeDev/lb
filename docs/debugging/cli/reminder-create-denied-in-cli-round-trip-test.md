# lb-cli `reminder_test` fails pre-existing: `Denied { tool: "reminder.create" }` over the real gateway

- **Date:** 2026-07-11 (logged during the updates-to-core release; failure predates the branch)
- **Area:** cli (reminders)
- **Status:** open — documented, deliberately not chased in the release session
- **Session:** [../../sessions/release/updates-to-core-release-session.md](../../sessions/release/updates-to-core-release-session.md)

## Symptom

`cargo test --workspace` has exactly one red test:
`lb-cli reminder_test::create_ls_show_update_rm_round_trips_over_the_real_gateway` — the CLI's
seeded principal is `Denied` on `reminder.create` against the spawned gateway.

## What we know

- Pre-existing: it fails identically on the branch base, before any release-session change; the
  reminders feature is not part of this branch's five features.
- Shape: a capability-fold gap for the CLI test login (the seeded member's caps don't include
  `mcp:reminder.create:call` on this path), not a reminders-engine regression — the host-level
  reminders tests are green.

## Next step (owner: the next reminders/cli session)

Reproduce with the CLI test harness, diff the folded caps at login vs `member_caps()`, and either
grant the reminder caps on the test seed or fix the fold. Add the regression test there.

This is the **one allowed failure** on the `node-v0.2.0` tag (release scope: "log it, don't chase
it, don't let it rot").
