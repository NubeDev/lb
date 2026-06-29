# Reminders scope — a scheduled trigger that fires an action

Status: **SHIPPED** (2026-06-29) — all open questions below were resolved at scope time and held in
the build. Promoted to [`public/reminders/reminders.md`](../../public/reminders/reminders.md); built
in [`sessions/reminders/reminders-session.md`](../../sessions/reminders/reminders-session.md).

A **reminder** is a durable, workspace-scoped schedule that fires an **action** when it comes
due: post a message to a channel, call an MCP tool, or emit a must-deliver outbox effect. It is
the user-facing front end for the scheduled-job machinery the platform already has — a
`run_at` + cron reactor over `lb-jobs` (§6.9) — wrapped in a record a human can create, pause,
and read. The schedule can be **one-shot or counted** ("run 10 times Tuesdays at 08:00, then
stop") or **recurring** ("every Monday and Sunday at 08:00, forever"), with an explicit
on/off switch.

## Goals

- Let a user (or an agent) schedule an action to fire later — once, N times, or recurring.
- Three action kinds at v1: **channel post** (inbox), **MCP tool call** (any capability), and
  **must-deliver effect** (outbox).
- Pause/resume (`enabled`) and a hard run-count cap (min 1) without deleting the reminder.
- Reuse the shipped scheduling machinery (the S6 cron reactor + `lb-jobs` + outbox) — add a
  record and a reactor, not a new scheduler.
- A first-class UI to author the schedule with a **best-in-class React cron builder** (most
  users don't read cron), and to pick/configure the action.

## Non-goals

- A general workflow/DAG engine — that is `rules/rule-chains-scope.md`. A reminder fires **one**
  action; chaining is the chains' job (a reminder *may* call a chain via the MCP-tool action).
- Natural-language scheduling ("remind me next Tuesday") — v1 takes a structured schedule; an
  NL→cron parser is a later layer over the same record.
- New delivery transports — channel/tool/outbox are the existing seams; no new egress here.
- A second datastore or a second scheduler (§3.2): the cron reactor already exists.

## Intent / approach

A reminder is **state** (a SurrealDB record), and its firing is **motion** driven by a durable
scan — the exact altitude of the shipped `react_to_approvals` / `relay_outbox` reactors
(`inbox-outbox.md`). The reminder record carries a `schedule` (cron string), an optional
`max_runs` counter, an `enabled` flag, a `next_attempt_ts`, and an **action** tagged union.

A new `react_to_reminders` durable scan finds every `enabled` reminder that is `due`
(`next_attempt_ts ≤ now`), enqueues one **`lb-jobs` job** per firing (so the work is durable,
resumable, and retried for free), dispatches the action inside the job, then advances the
reminder: decrement `max_runs` (stop at 0), compute the next `next_attempt_ts` from the cron
expression, or mark `done` if one-shot/exhausted. Computing "the next time after T" from a cron
string is the one new mechanical piece; everything downstream is shipped.

**Why this fits:** it is the same stateless-function-over-a-durable-set pattern already proven
twice (relay, approval reactor). The alternative — a long-lived in-process timer wheel — was
rejected: it holds durable state in a process (breaks rule 4 / stateless extensions), doesn't
survive a node restart, and doesn't work on a symmetric edge node that may be offline at the
fire time. A durable scan that catches up missed firings on the next pass is the only model
consistent with §3.

**Cron is the storage format, not the UX.** The record stores a standard 5-field cron string so
the existing reactor/`run_at` machinery (key-stack "Rule chains" row) consumes it unchanged. The
UI never asks a human to type cron: it renders a **React cron-builder** component
(see Open questions for the package choice) that reads/writes that string, so "every Mon & Sun
8am" is point-and-click and round-trips losslessly to `0 8 * * 0,1`.

## How it fits the core

- **Tenancy / isolation:** every reminder carries `ws`; the reactor scan is ws-scoped; a ws-B
  reactor never sees, fires, or advances a ws-A reminder. Mandatory isolation test.
- **Capabilities:** CRUD verbs are gated (`mcp:reminder.create:call`, `.update`, `.delete`,
  `.list`, `.get`). The **firing** re-checks the action's own capability under the reminder's
  stored principal — a reminder whose MCP-tool action calls `workflow.start_job` fires only if
  that principal still holds `mcp:workflow.start_job:call`; a revoked grant turns the firing into
  a logged deny, never a privilege-escalation backdoor. The deny path: create with no
  `reminder.create` grant → refused, no record; fire with a since-revoked action grant → deny
  logged, reminder stays scheduled (or dead-letters per the job's retry policy).
- **Placement:** either. The reactor runs wherever the node runs; an edge node fires its own
  workspace's reminders offline, catching up missed ones on reconnect. No `if cloud`.
- **MCP surface:** see API shape below.
- **Data (SurrealDB):** one `reminder:{id}` table (state). The firing's job is a `lb-jobs`
  record; a channel-post action writes an `lb_inbox::Item`; an outbox action writes an `Effect`.
- **Bus (Zenoh):** none owned here. A channel-post action's live copy rides the channel's
  existing bus subject (motion); the durable inbox item is the record. The reactor itself is a
  durable scan, not a bus consumer (a LIVE-query push is the same deferred optimization noted for
  the relay).
- **Sync / authority:** node-local record, workspace-authoritative like any other state; syncs
  on the normal path. Missed firings during an outage fire on the next scan (at-least-once,
  idempotent per below).
- **Secrets:** none directly. If an MCP-tool/outbox action needs a secret, it is mediated by the
  tool/`Target` it calls — the reminder stores a tool name + payload, never secret material.

### API shape

- **CRUD:** `reminder.create` / `reminder.update` / `reminder.delete` — the write verbs (one
  tool + capability + file each, FILE-LAYOUT). `update` covers pause/resume (`enabled`) and
  re-scheduling. These are bounded, always-fast single-record writes — synchronous, not jobs.
- **Get / list:** `reminder.get` (by id) and `reminder.list` (ws-scoped, filter by
  enabled/kind/channel). Read capability `mcp:reminder.list:call`.
- **Live feed:** N/A for v1. A reminder's *effects* are observable where they land (the channel
  feed, the job status, the outbox) — the reminder record itself changes only on fire/edit, for
  which `list`/`get` suffice. A `watch` is deferred with the LIVE-query reactor optimization.
- **Batch:** N/A. No bulk caller at v1; a future "snooze all" would be a bounded sync batch.

The **firing is itself the job** (API-shape §6.1: long/durable work is a job, never a blocking
call). The reactor enqueues a job; the job dispatches the action. A channel-post is a near-instant
inbox write; an MCP-tool/outbox action that can run long inherits the job's durability, retry,
and backoff. Must-deliver actions go through the **outbox**, not raw pub/sub (Durability rule).

## Example flow

"Every Monday & Sunday at 08:00, post *standup time* to #team."

1. User opens the reminder dialog, picks action **channel post** → channel `#team`, body
   `standup time`; sets the schedule with the React cron-builder (clicks Mon + Sun + 08:00),
   which writes `0 8 * * 0,1`; leaves `max_runs` empty (recurring) and `enabled` on.
2. UI calls `reminder.create` (caps-checked) → `reminder:{id}` persisted with `ws`,
   `next_attempt_ts` = next Sun/Mon 08:00 from the injected clock.
3. Sunday 08:00 the node's `react_to_reminders` scan finds it `due` and `enabled`, enqueues a
   `lb-jobs` job `kind="reminder-fire"` with the action payload.
4. The job dispatches: re-checks the channel-post capability under the reminder's principal,
   writes an `lb_inbox::Item` into `#team` (durable history) whose live copy rides the channel
   bus subject. The standup message appears for everyone watching `#team`.
5. The reactor advances the reminder: `max_runs` is unset so it stays `enabled`; recomputes
   `next_attempt_ts` to the next Monday 08:00. One firing, recorded; no double-fire on a re-scan
   (idempotency below).
6. Counted variant: had the user set `max_runs: 10`, step 5 would decrement to 9 and, at 0, set
   `enabled=false` / `status=done` — "run 10 times, then stop."

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability-deny** (required): create without `reminder.create` → refused, no record; a firing
  whose action grant was revoked → logged deny, no effect, no escalation.
- **Workspace-isolation** (required): a ws-B reactor scan never fires or advances a ws-A reminder;
  `list`/`get` in ws-B never return ws-A reminders. Tested across store + reactor.
- **Offline/sync:** a reminder due during a node outage fires on the next scan after recovery
  (at-least-once catch-up), exactly once (idempotent).

Key unit/integration/E2E cases (real store/bus/jobs/outbox — no mocks, §0):

- Cron "next after T" math, injected logical clock (testing §3): recurring multi-day, one-shot,
  DST-agnostic logical-clock behavior.
- `max_runs` counts down and stops at 0; `enabled=false` is skipped by the scan and resumes when
  re-enabled.
- **Idempotent firing:** a re-scan before `next_attempt_ts` advances fires nothing twice
  (deterministic per-firing job id derived from `(reminder_id, scheduled_ts)`, like the reactor's
  derived `job_id` — one scheduled instant, one job, one effect).
- Each action kind end-to-end against the real seam: channel post → real `lb_inbox` item in the
  channel; MCP-tool action → the real tool runs under the principal; outbox action → a real
  `Effect` relayed (reuse the outbox's real-socket test harness).

## Risks & hard problems

- **Cron semantics & "next after T".** Easy to get subtly wrong (multi-value fields, month/day
  rollover). Use a vetted cron crate for parsing/next-time; do not hand-roll. Keep it on the
  injected logical clock so tests are deterministic (no wall-clock, testing §3).
- **Missed-firing policy.** After an outage a recurring reminder may have skipped several slots.
  v1 fires **once** on catch-up and advances to the next future slot (no backfill storm); state
  this explicitly so "every minute" can't enqueue a thousand jobs after a long outage.
- **At-least-once → idempotency.** The scan is at-least-once; the deterministic per-firing job id
  is the dedup. Get it wrong and a reminder double-posts. This is the same discipline the relay
  and approval reactor already rely on.
- **Principal capture at fire time.** Storing *which* principal a firing runs as, and re-checking
  its caps at fire time (not create time), is the whole security model. A grant revoked after
  create must stop the firing.

## Open questions

All resolved at scope time — none open. Decisions (record in the session doc):

- **React cron-builder package — DECIDED: `react-js-cron`.** The most popular, maintained visual
  cron builder; round-trips a standard 5-field cron string losslessly. It is antd-based — wrap it
  in one component and restyle to the shell's Tailwind/shadcn tokens (don't pull antd into the
  global theme). Add the row to `key-stack.md`.
- **Rust cron crate — DECIDED: `croner`.** Modern, maintained, Vixie-cron compatible, with an
  explicit `find_next_occurrence(after: &DateTime, inclusive)` that takes a **supplied** time — so
  it runs on the injected logical clock (testing §3), never the wall clock. Add the row to
  `key-stack.md`.
- **Missed-firing backfill — DECIDED: fire-once-then-skip-to-next for v1.** On catch-up after an
  outage, fire once and advance `next_attempt_ts` to the next future slot (no backfill storm). A
  per-reminder backfill flag is a future addition, not v1.
- **Action payload schema — DECIDED: validate at fire time, best-effort check at create time.**
  The MCP-tool action stores `{tool, args}`. Create-time does a best-effort schema check for UX;
  the authoritative validation is at fire time (tool schemas evolve between create and fire).
- **Reactor placement — DECIDED: dedicated `react_to_reminders` scan in its own file** (one
  responsibility, FILE-LAYOUT), running on the same cadence as the existing S6 reactor pass — not
  folded into it.

## Related

- README `§6.9` (jobs), `§6.10` (outbox), `§3.3` (state vs motion), `§3.6` (workspace wall).
- `scope/jobs/jobs-scope.md` — the cron-representation open question this feature answers in
  practice; `run_at` + record-range scan is the mechanism.
- `scope/inbox-outbox/outbox-scope.md` + `public/inbox-outbox/inbox-outbox.md` — the durable-scan
  reactor pattern (`react_to_approvals`, `relay_outbox`) this reuses, and the must-deliver path.
- `scope/channels/channels-scope.md` — the channel-post action target (durable inbox + bus).
- `scope/rules/rule-chains-scope.md` — the cron-via-S6-reactor precedent; a reminder is the
  single-action sibling of a chain (and may *call* a chain via the MCP-tool action).
- `key-stack.md` — "Rule chains" / "Jobs" rows (cron via the reactor); add the `croner` cron crate
  + `react-js-cron` builder rows here.
