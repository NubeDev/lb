# Session — external-agent stall becomes pause-and-ask (user decides, in the UI)

**Date:** 2026-07-08 · **Topic:** external-agent / run-lifecycle + agent-dock · **Status:** shipped (green)

## The ask

Follow-up to `agent-run-hardening-session.md`: turn the no-progress stall reap from a terminal *fail*
into a **pause-and-ask** — suspend the run and let the user "keep going" or "stop", surfaced as an
**actionable UI** in the agent dock. "Do what's best long term."

## Design — on shipped rails, not a parallel mechanism

The platform already has a complete durable **suspend → decide → resume** machine
(`agent_decision` + `SuspensionOpened/Settled` + `agent.decide`) and a shipped run-control surface
(`pause_run`/`resume_run`/`stop_run` behind `mcp:agent.control:call`, HTTP `POST /runs/{job}/{op}`,
wired in the dock as `pauseRun`/`resumeRun`/`stopRun`). A stall is just another reason to suspend, so
the whole thing reuses those rails:

- **Stall = suspend, not fail.** `AcpRuntime::run`'s watchdog now `lb_jobs::suspend`s the run (leaving
  it `Suspended`/resumable from its cursor) and returns the new **`AgentError::Stalled`** instead of
  marking it `Failed`.
- **A distinct durable item.** The worker maps `Stalled` (via a new internal `DriveFault::Stalled`) to
  a **`kind:"agent_stalled"`** channel item (`AgentStalledPayload {goal, job, message}`) — NOT an
  `agent_error`. Posted under `a:<job>` like the other terminal items, so the dock reconciles it
  through the existing path.
- **"Keep going" = resume, "Stop" = stop.** Both are the already-shipped `resume_run` (re-enqueue +
  rehydrate from cursor, under the original asker's authority) and `stop_run` (cancel). No new control
  verb, no new persistence, no new SSE event — the stall rides the durable-item path the dock already
  reads.

Rejected: teaching the client SSE `RunEvent` type a new `suspended` variant + folding it live. The
durable-item path is simpler, is what the dock already reconciles (`pendingRun.ts`), and survives a
tab reload for free (the item is durable; a live event isn't).

Rejected: auto-stop after a grace window. The user chose "suspend + ask" with no auto-timer — a paused
run is cheap (no subprocess; it was reaped) and the user can always stop it. (Kept as an easy future
add if unattended paused runs ever pile up.)

## Changes

**Backend**
- `crates/host/src/agent/error.rs` — new `AgentError::Stalled` (non-terminal, distinct from
  `BadInput`/`Denied`).
- `role/external-agent/src/lib.rs` — the watchdog now `suspend`s + returns `Stalled` (was
  `complete(Failed)` + `BadInput`); removed the now-unused `STALL_MESSAGE`.
- `crates/host/src/channel/payload.rs` — new `KIND_AGENT_STALLED`, `AgentStalledPayload`,
  `ItemPayload::AgentStalled`, `agent_stalled_body`, and added the kind to the `parse_payload`
  allowlist.
- `crates/host/src/channel/agent_worker.rs` — new internal `DriveFault {Message, Stalled}`;
  `drive_run` returns it; `drive_queued_run` posts the `agent_stalled` item on `Stalled` (before the
  Paused/Stopped/Finished lifecycle match, since a stall is `Suspended` too). New `STALL_PROMPT` text.

**Frontend**
- `lib/channel/payload.types.ts` — `AgentStalledPayload` + union + `KINDS`.
- `features/agent-dock/pendingRun.ts` — folds `agent_stalled` into new `stalled`/`stalledText` (a stall
  is non-terminal; a later result/error naturally clears it).
- `features/agent-dock/AgentDock.tsx` — `active` excludes a stalled run (its stream ended); passes
  `stalled`/`stalledText` + `onKeepGoing`(resume)/`onStopStalled`(stop) to the status strip.
- `features/agent-dock/DockRunStatus.tsx` — a distinct amber **pause-and-ask card** (honest message +
  "Keep going" / "Stop" buttons, shadcn `<Button>`), taking precedence over the live phase.
- `features/channel/AgentCard.tsx` + `MessageItem.tsx` — the channel message-list surface renders a
  read-only "paused — open the dock to keep going or stop" notice for the new kind (render-only per
  FILE-LAYOUT; the dock owns the controls).

## Tests (all green)

- Backend: `role/external-agent/tests/no_progress_test.rs` — stall now leaves the job **Suspended**
  and returns `AgentError::Stalled` (was `Failed`/message). `crates/host/tests/channel_agent_worker_test.rs::a_stalled_run_posts_an_actionable_agent_stalled_prompt_and_stays_resumable`
  — a `StalledRuntime` stub → the worker posts a `kind:"agent_stalled"` item carrying the job + prompt,
  and the run stays `Suspended`. `payload.rs` unit: `agent_stalled_body` round-trips.
  - Also **revived** `channel_agent_worker_test.rs`, which was red on clean master (its `Claims`
    literal predated the `constraint`/`run_id` fields) — added the two `None` fields; 15 tests now run.
- Frontend: `pendingRun.test.ts` (+2: stall fold + clears on resume), `DockRunStatus.test.tsx` (+2:
  prompt renders/fires, yields to terminal). Full `agent-dock/` + `channel/AgentCard` suites: 69 green.
- `tsc --noEmit`: 0 errors. Lint: 0 NEW errors (4 pre-existing raw-`<button>` warnings in
  `DockRunStatus.tsx` predate this change; my additions use `<Button>`).

## Follow-ups

- Optional auto-stop for an unattended paused run (deferred by the user's choice).
- An iteration/token ceiling distinct from wall-time (the other half of the `run-lifecycle-scope.md`
  open item — this session did the no-progress half, now pause-and-ask).
- The channel (non-dock) surface is read-only for a stall today; if agent runs become common outside
  the dock, give `AgentCard` its own keep-going/stop wiring.
