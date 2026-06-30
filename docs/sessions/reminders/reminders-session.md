# Reminders — a scheduled trigger that fires an action (session)

- Date: 2026-06-29
- Scope: ../../scope/reminders/reminders-scope.md
- Public: ../../public/reminders/reminders.md
- Stage: post-S8 platform capability (scheduling plane); see STATUS.md
- Status: in-progress (kept as a live log — backend + frontend built and green; this doc is the working record)

## Goal

Build the **reminders** slice: a durable, workspace-scoped schedule (`reminder:{id}`) that fires
**one** action — channel post, MCP tool call, or must-deliver outbox effect — when it comes due,
driven by a dedicated durable scan (`react_to_reminders`) over the shipped `lb-jobs` + outbox
machinery. Cron is the **storage** format; the UI authoring surface is a visual cron builder. This is
the user-facing front end for the scheduled-job machinery the platform already has — a record + a
reactor, not a new scheduler (reminders scope "Intent / approach").

## Resolved decisions (from the scope's "Open questions") + where they landed

All five scope open questions were resolved at scope time; this records the *why* and the code site
each landed in.

- **Rust cron crate — `croner` 3.** Chosen for its explicit
  `Cron::find_next_occurrence(after: &DateTime, inclusive)` that takes a **supplied** time, so the
  whole crate runs on the **injected logical clock** (testing §3), never the wall clock —
  deterministic tests, correct semantics for replayed/seeded data. Hand-rolling "next after T"
  (multi-value fields, month/day rollover) is the scope's named hard problem; a vetted crate does it.
  Landed in `rust/crates/reminders/src/next_after.rs` (`next_after` + `is_valid`, `inclusive=false`
  so "next after T" is strictly future — the property that makes the reactor's advance idempotent).
  Dep: `rust/Cargo.toml` (`croner = "3"`). Key-stack row added.

- **React cron-builder — `react-js-cron`.** The most popular maintained visual cron builder;
  round-trips a standard 5-field cron string losslessly (most users don't read cron). It is
  antd-based, so it is wrapped in ONE component and antd is scoped to that subtree via a local
  `ConfigProvider` — antd is **not** pulled into the shell's global Tailwind/shadcn theme (the scope
  decision). Landed in `ui/src/features/reminders/CronBuilder.tsx`. Dep: `ui/package.json`
  (`react-js-cron ^6.0.2`). Key-stack row added.

- **Missed-firing backfill — fire-once-then-skip-to-next-future-slot (v1).** After an outage a
  recurring reminder may have skipped several slots; firing once per missed slot would enqueue a
  backfill storm ("every minute" → a thousand jobs). So on catch-up the reactor fires **once** for
  the due instant and advances `next_attempt_ts` to the slot **strictly after `now`** (not after the
  missed instant). Landed in `rust/crates/host/src/reminder/react.rs::advance`
  (`next_after(&reminder.schedule, now)`). A per-reminder backfill flag is a future addition.

- **Action payload schema — validate at fire time, best-effort at create time.** Tool schemas evolve
  between create and fire, so the authoritative validation is the firing re-entering the host
  `call_tool` chokepoint (which re-checks `mcp:{tool}:call` under the live schema). Create-time does
  only a best-effort structural check for UX (reject shapes that could never fire — empty
  channel/tool/target). Landed: best-effort in `rust/crates/host/src/reminder/create.rs`
  (`best_effort_check_action`, reused by `update.rs`); authoritative in
  `rust/crates/host/src/reminder/fire.rs::fire_mcp_tool`.

- **Reactor placement — a dedicated `react_to_reminders` scan in its own file.** One responsibility
  (FILE-LAYOUT), running on the same cadence as the existing S6 reactor pass — **not** folded into
  it. Same altitude as the shipped `react_to_approvals` / `relay_outbox` reactors: a stateless
  function over a durable set. Landed in `rust/crates/host/src/reminder/react.rs`.

## What shipped (end to end)

### `rust/crates/reminders` — the store side (`lb-reminders`, rhai/host-free, raw verbs)

Like `lb_inbox`/`lb_outbox`/`lb_jobs`: the record + raw store verbs + the cron math, holding **no
authorization** and no host seams. The host `reminder` service runs these *after* `caps::check`.

- `model.rs` — the `Reminder` record (`schedule`, `max_runs`/`runs`, `enabled`, `status`
  Active/Done, `action`, `principal_sub`, `next_attempt_ts`, soft-delete `deleted`, injected logical
  `ts`) and the `Action` tagged union (`ChannelPost` / `McpTool` / `Outbox`).
- `next_after.rs` — `next_after` (croner "next after T" on the injected clock) + `is_valid` (the
  create-time best-effort cron check).
- `save.rs` / `load.rs` — upsert (create + update share it; idempotent on `id`) / read-by-id
  (`None` cross-workspace — isolation is the namespace).
- `scan.rs` — `list` (every non-deleted reminder, for the UI) and `due` (the reactor's subset:
  `active` AND `enabled` AND `next_attempt_ts ≤ now`, sorted by `next_attempt_ts`).
- `error.rs` — `ReminderError` (opaque `Denied`, `NotFound`, `BadCron`, `BadInput`, `Store`).

### `rust/crates/host/src/reminder` — the host service (auth + orchestration, one verb per file)

- `authorize.rs` — `authorize_reminder`: the CRUD gate via the shared `lb_mcp::authorize_tool`
  chokepoint (`mcp:reminder.<verb>:call`, workspace-first), opaque `Denied`. Independent of the caps
  the firing re-checks.
- `create.rs` — `reminder_create`: cap gate → best-effort cron + action + `max_runs≥1` validation →
  compute first `next_attempt_ts` (next slot strictly after `now`, never fire at create) → persist
  under the caller's principal (the stored principal the firing re-resolves). Synchronous, **not a
  job**.
- `update.rs` — `reminder_update` + `ReminderPatch`: pause/resume (`enabled`), reschedule
  (`schedule`/`max_runs`), and action edits; a reschedule or a resume re-anchors `next_attempt_ts` to
  the next future slot (no backfill on resume) and re-activates a `Done` reminder.
- `delete.rs` — `reminder_delete`: idempotent tombstone.
- `get.rs` — `reminder_get` / `reminder_list` (the gated reads).
- `tool.rs` — `call_reminder_tool`: the MCP bridge (create/update/delete/get/list), JSON in/out,
  camelCase wire view (`reminder_json`), each verb's own gate first.
- `fire.rs` — `fire_reminder`: the firing dispatcher (the job's body). Re-resolves the stored
  principal's CURRENT caps from the durable grant store (`resolve_caps`) then dispatches each action
  kind against its **real seam**:
  - **ChannelPost** → re-checks `bus:chan/{channel}:pub`, writes a durable `lb_inbox::Item` (id
    stable on `(reminder, scheduled_ts)`), best-effort live bus echo;
  - **McpTool** → re-enters `call_tool` (re-checks `mcp:{tool}:call` under the live schema);
  - **Outbox** → `enqueue_outbox` (re-checks `mcp:outbox.enqueue:call`, stages a pending `Effect`).
  Also owns `fire_job_id(reminder_id, scheduled_ts)` (the deterministic per-firing id) and
  `FIRE_KIND = "reminder-fire"`.
- `react.rs` — `react_to_reminders`: the dedicated durable scan. For each `due` reminder: compute
  the stable `fire_job_id`; **skip if that job already exists** (`lb_jobs::load` — the idempotent
  no-op, one scheduled instant → one job → one effect); else record the durable firing job BEFORE
  dispatch (a crash mid-fire leaves an idempotent marker), dispatch, and on `Ok` `advance`
  (bump `runs`; mark `Done` at `max_runs` / one-shot, else recompute `next_attempt_ts` to the next
  slot strictly after `now`). A `Denied` firing is **logged and left scheduled** (not advanced — the
  job exists, so a re-scan won't re-fire). Returns a `ReactorPass { fired, skipped, denied }` tally.

### `ui/` — the authoring surface

- `ui/src/features/reminders/CronBuilder.tsx` — the `react-js-cron` wrapper (antd scoped locally).
- `ui/src/features/reminders/ActionEditor.tsx` — pick + configure the one action (channel-post /
  mcp-tool / outbox), shadcn fields.
- `ui/src/features/reminders/useReminders.ts` — the list/CRUD hook.
- `ui/src/lib/reminders/reminders.api.ts` — one call per `reminder.*` verb over the host-mediated
  `POST /mcp/call` bridge (ws + principal from the token; each verb re-checked server-side).
- `ui/src/lib/reminders/reminders.types.ts` — the `Reminder` / `ReminderAction` wire types.

## Decisions & alternatives (implementation-level)

- **The firing is the job; the CRUD verbs are not.** `reminder.create/update/delete/get/list` are
  bounded single-record writes → synchronous (API-shape §6.1). The firing is the `lb-jobs` job, so a
  long MCP-tool/outbox action inherits durability/retry/backoff for free. *Rejected:* a long-lived
  in-process timer wheel — it holds durable state in a process (breaks rule 4), doesn't survive a
  restart, and can't fire offline on a symmetric edge node (reminders scope). A durable scan that
  catches up on the next pass is the only §3-consistent model.
- **Principal captured at create, caps re-resolved at fire.** The record stores `principal_sub`, not
  a frozen cap set. The firing re-resolves caps from the durable grant store
  (`fire.rs::resolve_fire_principal`), so a grant revoked after create stops the firing at the
  action's own gate — never an escalation backdoor. This is the whole security model; the deny-test
  proves a revoked grant turns the firing into a logged deny with no effect.
- **A denied firing leaves the reminder scheduled, not advanced.** The job for that instant exists
  (recorded before dispatch), so a re-scan skips it — no retry storm, no re-fire of the denied
  instant, no double-effect. The reminder simply waits.
- **ChannelPost is decoupled from `channel::post`.** The firing re-checks the same
  `bus:chan/{channel}:pub` gate but writes the `lb_inbox::Item` directly + a best-effort bus echo, so
  a reminder firing rides only the stable record + cap seams (the record is the truth, §3.3; a bus
  hiccup never fails a durable firing).

## Tests

Mandatory categories covered (real embedded SurrealDB + in-proc Zenoh — no mocks, §0):

- **Capability-deny:** `reminders_mcp_test.rs::each_verb_is_denied_without_its_grant` (per CRUD verb,
  opaque, no record); `reminders_reactor_test.rs::a_revoked_action_grant_is_a_logged_deny_with_no_effect`
  (a since-revoked action grant → logged deny, no inbox item, reminder left scheduled, no re-fire).
- **Workspace-isolation:** `reminders_mcp_test.rs::workspace_isolation_list_and_get_never_cross_the_wall`
  (ws-B list/get never see ws-A); `reminders_reactor_test.rs::a_ws_b_reactor_never_fires_or_advances_a_ws_a_reminder`
  (a ws-B reactor pass fires/advances nothing in ws-A; ws-A's own pass then fires it).
- **Offline/sync:** `reminders_reactor_test.rs::a_due_during_outage_fires_exactly_once_on_catch_up`
  (a slot passed during an outage → exactly one catch-up fire, then advance to the next future slot —
  no backfill storm).

Key behaviour cases:

- `reminders_mcp_test.rs`: `create_get_list_update_delete_round_trip` (the full CRUD through the real
  `call_tool` bridge, incl. computed `nextAttemptTs`), `bad_cron_at_create_is_bad_input_not_denied`
  (malformed schedule = author feedback, not a denial), `unknown_verb_is_opaque_denied_at_the_gate`.
- `reminders_reactor_test.rs`: each action kind against its real seam —
  `channel_post_firing_writes_a_real_inbox_item_and_advances`,
  `mcp_tool_firing_runs_the_real_tool_under_the_stored_principal`,
  `outbox_firing_enqueues_a_real_effect_relayed_via_the_outbox`; idempotency
  (`a_re_scan_before_advance_fires_nothing_twice`); `max_runs_counts_down_to_done`;
  `disabled_is_skipped_and_resumes_at_the_next_future_slot`; and the cron "next after T" multi-day
  math on the injected clock (`recurring_multi_day_schedule_on_the_injected_clock`,
  Mon+Sun 08:00 → Mon 08:00 then Sun 08:00). Plus the `lb-reminders` unit tests for the cron math.

## Test output

### Backend — `lb-reminders` unit + `lb-host` integration (real `mem://` store / bus / caps / jobs / outbox)

```
$ cargo test -p lb-reminders
running 6 tests
test bad_cron_is_rejected_and_valid_is_accepted ... ok
test one_shot_anchor_picks_the_single_next_slot ... ok
test recurring_weekly_monday_from_midnight ... ok
test every_minute_advances_by_sixty_seconds ... ok
test recurring_multi_day_field_sun_and_mon ... ok
test strictly_after_is_inclusive_false ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test -p lb-host --test reminders_mcp_test --test reminders_reactor_test
running 5 tests   # reminders_mcp_test
test unknown_verb_is_opaque_denied_at_the_gate ... ok
test each_verb_is_denied_without_its_grant ... ok
test bad_cron_at_create_is_bad_input_not_denied ... ok
test workspace_isolation_list_and_get_never_cross_the_wall ... ok
test create_get_list_update_delete_round_trip ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 10 tests   # reminders_reactor_test
test outbox_firing_enqueues_a_real_effect_relayed_via_the_outbox ... ok
test channel_post_firing_writes_a_real_inbox_item_and_advances ... ok
test max_runs_counts_down_to_done ... ok
test recurring_multi_day_schedule_on_the_injected_clock ... ok
test disabled_is_skipped_and_resumes_at_the_next_future_slot ... ok
test a_ws_b_reactor_never_fires_or_advances_a_ws_a_reminder ... ok
test a_re_scan_before_advance_fires_nothing_twice ... ok
test a_revoked_action_grant_is_a_logged_deny_with_no_effect ... ok
test mcp_tool_firing_runs_the_real_tool_under_the_stored_principal ... ok
test a_due_during_outage_fires_exactly_once_on_catch_up ... ok
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Frontend — `RemindersView` against a REAL spawned gateway (`vitest.gateway.config.ts`, no fake backend)

```
$ npx vitest run --config vitest.gateway.config.ts src/features/reminders/RemindersView.gateway.test.tsx
 ✓ src/features/reminders/RemindersView.gateway.test.tsx (3 tests)
   ✓ RemindersView (real gateway) > creates a reminder via the real path and lists it
   ✓ RemindersView (real gateway) > pauses and resumes a reminder via the real update verb
   ✓ RemindersView (real gateway) > deletes a reminder (tombstone) via the real delete verb
 Test Files  1 passed (1)
      Tests  3 passed (3)
```

(The per-verb capability-deny + workspace-isolation are proven server-side in `reminders_mcp_test.rs`
above; the frontend test proves the UI drives the real `reminder.*` verbs, not a fake.)

## Debugging

**`reminders/ts-unit-mismatch-cron-search-limit.md`** — the first real run of the frontend gateway
test crashed every `reminder.create` with `bad input: CronScheduler time search limit exceeded`. Root
cause: the host's logical `ts`/`now` is **seconds** since the epoch (the unit `next_after` feeds
croner), but `useReminders` passed `Date.now()` (**milliseconds**, ~1.7e12). The host fed that
millisecond value to `next_after`, which converted it to a year-~55000 instant; croner aborted its
forward search. **Fix:** convert to seconds at the seam (`nowSecs() = Math.floor(Date.now()/1000)` in
`ui/src/features/reminders/useReminders.ts`). **Regression:** the gateway test
`creates a reminder via the real path and lists it` asserts `nextAttemptTs > 0` and round-trips the
schedule — it **failed-before** (the create threw) and **passes-after** the unit fix. Logged in
`../../debugging/README.md`.

## Public / scope updates

- Promoted the shipped truth to `../../public/reminders/reminders.md`.
- Appended a reminders entry to `../../public/SCOPE.md`.
- Added two `../../key-stack.md` rows (`croner`, `react-js-cron`).
- The scope's "Open questions" were already all resolved; recorded each decision's *why* + code site
  above.

## Dead ends / surprises

- None notable. The reactor reused the proven approval/relay altitude exactly; the only genuinely new
  mechanical piece was `next_after` (the cron math), isolated behind one crate fn so it is unit-tested
  in `lb-reminders` and exercised end to end on the injected clock in the host reactor tests.

## Follow-ups

- A per-reminder **backfill** flag (v1 is fire-once-then-skip), an NL→cron parser layer over the same
  record, and a `reminder.watch` live feed (deferred with the LIVE-query reactor optimization) — all
  noted in the scope.
- STATUS.md: update the reminders slice state.
