# Reminders (public)

Status: **SHIPPED** (2026-06-29). Scope: `../../scope/reminders/reminders-scope.md`. Session:
`../../sessions/reminders/reminders-session.md`.

A **reminder** is a durable, workspace-scoped schedule that fires **one** action when it comes due:
post a message to a channel, call an MCP tool, or emit a must-deliver outbox effect. It is the
user-facing front end for the scheduling machinery the platform already has — a record + a durable
scan over `lb-jobs` + the outbox, **not** a new scheduler (§3.2). A reminder is one-shot, counted
("run 10 times Tuesdays at 08:00, then stop"), or recurring forever, with an explicit on/off switch.
Reached the same way as everything else: workspace-first, capability-gated MCP verbs (rule 7).

## What it is — state vs motion

The reminder is **state**: a SurrealDB record at `reminder:{id}` within a workspace namespace, so a
schedule survives a crash and a node restart (`crates/reminders` = `lb-reminders`, the store side,
holding no authorization). Its firing is **motion** driven by a durable scan — the exact altitude of
the shipped `react_to_approvals` / `relay_outbox` reactors. A long-lived in-process timer wheel was
**rejected**: it holds durable state in a process (breaks stateless-extensions), doesn't survive a
restart, and can't fire offline on a symmetric edge node. A durable scan that catches up on the next
pass is the only model consistent with §3.

## The record (data model, one datastore)

`reminder:{id}` carries:

- `schedule` — a standard **5-field cron string** (the storage format; the UI never asks a human to
  type it). Consumed by the cron "next after T" math (`croner`) on the **injected logical clock**.
- `max_runs` (`Some(n≥1)` = fire at most n times then stop; `None` = recurring forever) + `runs`
  (count so far; `runs == max_runs` ⇒ `Done`).
- `enabled` — the on/off switch (pause/resume without deleting).
- `status` — `Active` (the reactor considers it) or `Done` (terminal; kept for history, never fired
  again, not deleted).
- `action` — a tagged union: `ChannelPost{channel, body}` / `McpTool{tool, args}` /
  `Outbox{target, action, payload}`. **One** action per reminder (chaining is the rule-chains' job; a
  reminder *may* call a chain via the `McpTool` action).
- `principal_sub` — the creator's identity. The firing re-resolves its caps at fire time.
- `next_attempt_ts` — the next instant to fire (computed from `schedule`).
- `deleted` — a soft-delete tombstone (idempotent delete; tombstoned rows never fire/list).

The firing's job is an `lb-jobs` record; a channel-post writes an `lb_inbox::Item`; an outbox action
writes an `Effect`. No second datastore (§3.2).

**Cron is the storage format, not the UX.** The record stores the cron string so the reactor consumes
it unchanged; the UI renders a visual builder that reads/writes that string, so "every Mon & Sun 8am"
is point-and-click and round-trips losslessly to `0 8 * * 0,1`.

## MCP surface — `reminder.*`

The CRUD verbs are bounded, always-fast single-record writes — **synchronous, not jobs** (the firing
is the job). One tool + capability + file each (FILE-LAYOUT).

| Verb | Cap | Does |
|---|---|---|
| `reminder.create {id, schedule, max_runs?, action, ts}` | `mcp:reminder.create:call` | best-effort-validate cron + action, compute the first `next_attempt_ts` (next slot **after** `now` — never fires at create), persist under the caller as the stored principal. |
| `reminder.update {id, schedule?, max_runs?, enabled?, action?, ts}` | `mcp:reminder.update:call` | partial update — covers **pause/resume** (`enabled`) and **reschedule**; a reschedule/resume re-anchors `next_attempt_ts` to the next future slot (no backfill on resume) and re-activates a `Done` reminder. |
| `reminder.delete {id, ts}` | `mcp:reminder.delete:call` | idempotent tombstone. |
| `reminder.get {id}` | `mcp:reminder.get:call` | read one (workspace-walled; `null` if absent/deleted). |
| `reminder.list {}` | `mcp:reminder.list:call` | every non-deleted reminder in the workspace. |

A malformed schedule is `BadInput` (author feedback), **not** a denial — the caller is authorized;
the input is just wrong. Any capability/membership denial is opaque `Denied`. A live feed and batch
verbs are explicit v1 non-goals (a reminder's *effects* are observable where they land).

## The reactor — `react_to_reminders` (firing semantics)

A dedicated durable scan in its own file (one responsibility), running on the same cadence as the S6
reactor pass — **not** folded into it. One pass over a workspace at logical time `now`:

1. Take every `due` reminder (`active` AND `enabled` AND `next_attempt_ts ≤ now`).
2. Compute the deterministic per-firing job id `fire_job_id(reminder_id, scheduled_ts)`; **if that
   `lb-jobs` job already exists, skip** (the idempotent no-op).
3. Else record the firing job (`kind="reminder-fire"`) **before** dispatch — a crash mid-fire leaves
   an idempotent marker.
4. Dispatch the action under the stored principal (below).
5. On success, **advance**: bump `runs`; mark `Done` (a one-shot, or `runs` reached `max_runs`,
   which also flips `enabled=false`); else recompute `next_attempt_ts` to the next slot strictly
   after `now`.

A pass returns a `{ fired, skipped, denied }` tally. Triggers fire wherever the node runs (no
`if cloud`); an edge node fires its own workspace's reminders offline.

## Idempotency + missed-firing policy

- **One scheduled instant → one job → one effect.** The per-firing job id is deterministic on
  `(reminder_id, scheduled_ts)`; the existence check is the dedup — exactly the discipline the relay
  and approval reactor rely on. A re-scan before the advance moves `next_attempt_ts` fires nothing
  twice. The scan is at-least-once; the stable id is the idempotency.
- **Fire-once-then-skip-to-next-future-slot (v1).** After an outage a recurring reminder may have
  skipped several slots. The reactor fires **once** for the due instant and advances to the next slot
  **strictly after `now`** (not after the missed instant), so "every minute" can't enqueue a thousand
  jobs after a long outage. A per-reminder backfill flag is a future addition.

## The security model — principal capture at fire time

The record stores `principal_sub`, **not** a frozen cap set. The firing **re-resolves** that
principal's CURRENT caps from the durable grant store and re-checks the action's **own** capability
under them:

- **ChannelPost** re-checks `bus:chan/{channel}:pub`, then writes a durable `lb_inbox::Item` (live
  bus echo is best-effort motion; the record is the truth).
- **McpTool** re-enters the host `call_tool` chokepoint, which re-checks `mcp:{tool}:call` under the
  **live** tool schema (authoritative validation at fire time — schemas evolve between create and
  fire; create-time does only a best-effort UX check).
- **Outbox** re-checks `mcp:outbox.enqueue:call` and stages a pending `Effect` (the relay owns
  delivery; must-deliver rides the transactional outbox, never raw pub/sub).

A grant **revoked after create** turns the firing into a **logged deny** with no effect — never a
privilege-escalation backdoor. A denied firing leaves the reminder **scheduled** (not advanced); the
job for that instant already exists, so a re-scan does not re-fire it. The CRUD gate
(`mcp:reminder.<verb>:call`) is independent: being allowed to *create* a reminder never implies the
action it fires. The workspace wall holds at the scan — `due` selects the namespace, so a ws-B
reactor never sees, fires, or advances a ws-A reminder.

## The UI

The authoring surface (`ui/src/features/reminders/`, `ui/src/lib/reminders/`):

- **CronBuilder** — a thin wrapper around `react-js-cron` (the visual cron builder; most users don't
  read cron). It is antd-based, so antd is scoped to that subtree via a local `ConfigProvider` and
  **not** pulled into the shell's global Tailwind/shadcn theme.
- **ActionEditor** — pick + configure the one action (channel-post / mcp-tool / outbox) with shadcn
  fields.
- **`reminders.api.ts`** — one call per `reminder.*` verb over the host-mediated `POST /mcp/call`
  bridge, so the workspace + principal come from the session token and each verb is re-checked
  server-side (the hard wall, §7).

## Tests (the gate — all green)

Real embedded SurrealDB + in-proc Zenoh + real `lb-jobs`/inbox/outbox — **no mocks** (§0). Categories
covered:

- **`lb-reminders` unit** — the cron "next after T" math on the injected clock.
- **`reminders_mcp_test.rs`** — the full create/get/list/update/delete round-trip through the real
  `call_tool` bridge; **capability-deny per verb** (opaque, no record); **workspace-isolation** at
  list/get; bad-cron = `BadInput`; an unknown verb opaque-denied at the gate.
- **`reminders_reactor_test.rs`** — each action kind end-to-end against its **real seam** (channel
  post → a real `lb_inbox` item; MCP tool → the real tool under the stored principal; outbox → a real
  `Effect` relayed); **idempotent** re-scan (no double-fire); `max_runs` counts down to `Done`;
  `enabled=false` skipped and resumes at the next future slot; **offline/sync** (a slot passed during
  an outage fires exactly once on catch-up, no backfill); **capability-deny** at firing (a revoked
  action grant → logged deny, no effect, left scheduled); **workspace-isolation** across store +
  reactor; and the recurring multi-day cron math (Mon+Sun 08:00) on the injected clock.
