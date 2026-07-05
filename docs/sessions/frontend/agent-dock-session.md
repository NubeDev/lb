# Frontend — the agent dock (persistent, page-context-aware AI side panel) (session)

- Date: 2026-07-05
- Scope: ../../scope/frontend/agent-dock-scope.md
- Stage: post-S8 (data plane shipped) — a UI slice over shipped channel + agent + run-stream pieces
- Status: done

## Goal

Build the **agent dock** end to end: a persistent, resizable, non-modal right panel — shell-mounted so
it survives navigation — that talks to the workspace's **active agent** over **durable channel-backed
history**, carries **per-message page context** into the run, and shows **six honest run states** from
folded run events (never a bare spinner). A THIN CLIENT over three shipped pieces (channels, the durable
channel agent worker, the run-event SSE stream) plus one small additive host seam. Exit criterion: the
scope's Testing plan (mandatory capability-deny + workspace-isolation, session lifecycle, host context
injection, streaming state machine) green against the REAL gateway.

## What changed

**Host (the one additive seam — no new verb / cap / table):**
- `rust/crates/host/src/agent/page_context.rs` (NEW) — `fence_into_goal(goal, context)`: the ONE place
  a client-reported `{surface, path, search}` object is fenced into a run's goal as *untrusted,
  client-reported context*, with a hard **4 KB** cap (`MAX_CONTEXT_BYTES`) that **rejects** (not
  truncates) an oversize object. Absent context ⇒ byte-identical goal.
- `agent/dispatch.rs::invoke_via_runtime` — threads `context: Option<&Value>` and calls the fence after
  substrate is baked. This is the seam **both** front doors reach, so parity is structural, not copied.
- `channel/payload.rs` `AgentPayload.context` + `channel/agent_job.rs` `ChannelAgentJob.context` (both
  `#[serde(default, skip_serializing_if)]` — absent drops off the wire); `channel/agent_worker.rs`
  threads it from the payload → durable enqueue record → `drive_run` → the fence.
- `agent/route.rs` `AgentInvokeRequest.context` + `role/gateway/routes/agent_invoke.rs`
  `InvokeRequest.context` (parity door); `agent/serve.rs` + `invoke_remote.rs` pass it through.
- Re-exported `fence_into_goal` + `MAX_CONTEXT_BYTES` from the crate root.

**UI shared (additive, mirrors the host):**
- `lib/channel/payload.types.ts` — `PageContext` type + `context?` on `AgentPayload`; `encodeAgent`
  takes an optional `context`.
- `features/channel/useChannel.ts` — `postAgent(goal, runtime?, context?)`.
- `lib/channel/run.stream.ts` — `openRunStream(job, onEvent, onError?)` (additive `onError` for the
  dock's degrade/error states; jsdom has no EventSource so this only fires in a real browser).
- `features/channel/useChannels.ts` — filters `dock-*` OUT of the channels surface.

**UI feature (`features/agent-dock/`, one responsibility per file):**
- `dockId.ts` — mint/parse/filter the `dock-{user-slug}-{ulid}` id grammar.
- `pageContext.ts` + `PageContextProvider.tsx` — build the router-derived context; provider with an
  optional `source` override seam (decision 3).
- `useStallTimer.ts` (+ pure `computeStall`) — elapsed + 15 s stall state machine.
- `dockRunState.ts` — the pure six-state fold; `useDockRun.ts` wires the live stream + stall + phase,
  degrading honestly when `mcp:agent.watch:call` is absent.
- `useDockSessions.ts` (picker list + current + new-session) / `useDockSession.ts` (reuse `useChannel`,
  capture context per ask) / `pendingRun.ts` (newest run + terminal signals from items).
- `useDockChrome.ts` (open/width persistence + mobile floor via `useIsMobile`), `useDockHotkey.ts`
  (`mod+j`, one listener), `DockLauncher.tsx` (StatusBar button + run pip).
- `AgentDock.tsx` + presentation (`DockComposer`, `DockSessionPicker`, `DockContextCaption`,
  `DockRunStatus`).
- Mounted once in `features/routing/RoutedShell.tsx` beside `<Outlet/>` (page reflows, survives nav);
  launcher passed to `StatusBar` via a new `trailing` slot; `Escape` closes + returns focus.

## Decisions & alternatives

- **Session id separator `-`, not `.`** — the cap grammar splits on `/` AND `.`, and members hold
  `bus:chan/*:pub` (single `*` = one segment). A dotted id would be **denied** on create-on-post for an
  ordinary member. A dash keeps the id one segment. (Scope updated; see "Resolved during implementation"
  #1.) This was found by a red gateway test (the post silently 403'd), not by reading — logged below.
- **`@nube/panel` PRIMITIVES, not its `Panel`** — `Panel` wraps a modal Radix `Sheet` (the overlay the
  scope rejected). Built the frame from `useResizable` + `ResizeHandle` in a shell flex slot (non-modal,
  reflows). Scope updated (#2).
- **`PageContextProvider` `source` prop** — realizes the decision-3 override seam AND makes the dock
  router-free in tests. Chose this over mounting a full memory router per test.
- **Stream error ⇒ degrade, never Error** — a watch-stream 403/drop means "no live deltas"; the durable
  `agent_result` still renders. The Error phase comes only from a durable `agent_error` or a channel
  post/auth rejection (`hasError`), per the scope's honest-degrade rule.

## Tests

Real store/bus/gateway; no mocks/fakes (rule 9). The only permitted fake is the model **provider** HTTP
(a capturing/scripted `Provider`), used to inspect the assembled prompt.

**Rust — host `agent_page_context_test.rs` (3/3):**
```
running 3 tests
test oversize_context_is_rejected_before_any_model_call ... ok
test absent_context_is_byte_identical_to_today ... ok
test context_is_fenced_into_the_prompt_as_untrusted ... ok
test result: ok. 3 passed; 0 failed
```
**Rust — gateway `agent_invoke_route_test.rs` (5/5, incl. the two new context cases):**
```
test invoke_accepts_the_optional_page_context ... ok
test invoke_rejects_an_oversize_page_context ... ok
test invoke_without_the_cap_is_denied ... ok
test the_run_is_keyed_to_the_tokens_workspace ... ok
test granted_invoke_drives_the_active_agent ... ok
test result: ok. 5 passed; 0 failed
```
**Rust — units:** `agent::page_context` 5/5, `channel::agent_job` 3/3, `channel::payload` 13/13; the
affected `invoke_via_runtime` callers (`agent_default_runtime`/`runtime_seam`/`active_model`/
`in_house_wiring`/`external_substrate`) 25/25 green after threading the `context` arg.
`cargo build --workspace` + `cargo fmt` clean.

**UI unit (30/30):**
```
✓ src/features/agent-dock/dockRunState.test.ts   (10 tests)   — the six-state fold over REAL RunEvents
✓ src/features/agent-dock/dockId.test.ts         (8 tests)    — mint/slug/filter; id is one cap segment
✓ src/features/agent-dock/useStallTimer.test.ts  (5 tests)    — the stall state machine
✓ src/features/agent-dock/pendingRun.test.ts     (4 tests)    — newest run + terminal signals
✓ src/features/agent-dock/pageContext.test.ts    (3 tests)    — tenant-stripped surface/path/search
```
**UI gateway `AgentDock.gateway.test.tsx` (7/7, real spawned gateway):**
```
✓ first send CREATES the dock channel and the caption shows the captured context
✓ drives the run to a durable answer (Done — the message of record)
✓ history restores after a remount (durable — never anywhere but SurrealDB)
✓ new session mints a SECOND dock channel; the old stays reopenable
✓ the channels surface EXCLUDES dock-* sessions
✓ MANDATORY capability-deny: no bus:chan/*:pub → the post 403s and the dock shows an error
✓ MANDATORY workspace-isolation: a ws-B token cannot read a ws-A dock channel's history
Test Files 1 passed (1) · Tests 7 passed (7)
```

Mandatory categories covered: **capability-deny** (no pub → 403 error state; the no-`agent.watch`
degrade is proven by the `useDockRun` degrade path + `openRunStream` onError — jsdom can't open the live
stream, so the *transport* watch-cap check is the gateway's, exercised in Rust). **Workspace-isolation**
(ws-B can't read ws-A dock history; the `/runs/{job}/stream` ws-wall is the run-stream route's, unchanged
and already isolation-tested). Offline/sync + hot-reload: n/a (no extension instance, no new durable
surface). **Streaming state machine** is asserted as a unit over real `RunEvent` shapes because **jsdom
has no EventSource** (the established pattern — the live SSE folding is a Rust-transport concern; the UI
proves the durable path end to end).

## Debugging

- `debugging/frontend/dock-channel-id-dotted-cap-deny.md` — dotted `dock.` ids silently 403 on
  create-on-post for ordinary members (cap grammar splits on `.`; single `*` matches one segment).
  Root cause + fix (`-` separator) + regression test (`dockId.test.ts` asserts the minted id carries no
  `.`/`/`; the gateway create-on-post test would fail for a dotted id).

## Public / scope updates

- Promoted to `public/frontend/frontend.md` — the "Agent dock" section (what shipped, the seams, the
  states, the honest degrade, the id convention).
- Scope open questions closed; three implementation contradictions resolved + recorded in the scope's
  "Resolved during implementation" block (separator, panel primitives, provider seam).

## Skill docs

n/a: no new MCP verb or route (the dock consumes existing channel/agent/run surfaces). The scope itself
states "Skill doc: N/A" — the drivable surfaces already belong to their topics.

## Dead ends / surprises

- The scope's `@nube/panel` `Panel` component is modal (Radix Sheet) — the exact overlay the scope's own
  Intent rejected. Used its non-modal primitives instead.
- A mid-session file-collision with a concurrent AI session reverted the shared tracked-file edits; the
  user restored them. New files (page_context.rs, the whole `agent-dock/` feature) were never affected.
- Pre-existing red NOT from this slice: `radius-scale.guard` flags a bare `rounded` in the other
  session's in-flight `TemplateSourceField.tsx`; `sqlSource.gateway`/`SystemView.gateway` fail on clean
  master (per prior notes). None touch agent-dock.

## Follow-ups

- Per-channel membership/ACL (dock history is workspace-visible today — the honest v1 posture, stated in
  the scope's Risks) belongs to the channels topic; the dock inherits it for free when it lands.
- A live browser (non-jsdom) e2e could assert the Sent→Working→Answering deltas against real SSE frames;
  today that folding is unit-proven and the transport is Rust-proven.
- STATUS.md updated: agent dock shipped.

---

## Follow-up (2026-07-05): run controls — stop / pause / resume

The dock had no way to stop/pause/resume a run. Added it end to end as a thin, authorized front
door onto the shipped run-job lifecycle (`lb_jobs`) — **one new cap, one new route, no new table.**

### What changed
- **Host** `rust/crates/host/src/run_events/control.rs` (NEW): `stop_run` (→ `lb_jobs::cancel`),
  `pause_run` (→ `suspend`), `resume_run` (→ `unsuspend` + re-`create` the retired channel enqueue
  job so the reactor re-drives from the cursor). Each gates `mcp:agent.control:call` workspace-first
  (opaque `Denied`). Exported from the crate root.
- **Loop** `agent/step.rs::is_paused` (mirrors `is_cancelled`) + a pause check in `agent/run.rs` at
  the turn boundary: a paused run emits `RunFinish(Suspended)` and returns with the transcript/cursor
  intact (restartable), checked AFTER cancel (terminal wins).
- **Worker** `channel/agent_worker.rs`: after the drive, re-read the run-job status
  (`run_lifecycle_state`) to classify Finished / Paused / Stopped — a paused run posts **nothing**
  (not done yet); a stopped run posts the honest `run stopped` `agent_error`; finished posts the
  answer as before.
- **Cap** `mcp:agent.control:call` added to `member_caps()` + an `agent.control` catalog descriptor.
- **Gateway** `routes/run_control.rs` (NEW): `POST /runs/{job}/{op}` (`cancel|stop|pause|resume`),
  header-authed, `204` on success, opaque `403` on deny, `400` on unknown op / bad state.
- **UI** `lib/channel/run.control.ts` (`stopRun`/`pauseRun`/`resumeRun` → `agent_control` verb →
  `POST /runs/{job}/{op}`); Pause/Stop/Resume affordances in `DockRunStatus.tsx`; optimistic
  `paused` state + handlers in `AgentDock.tsx`.

### Decisions
- **Pause = `Suspended`, reusing the existing lifecycle state** (not a new Job field). The loop's
  pause-check reads `Suspended`; resume flips to `Running` FIRST (so the loop doesn't re-pause) then
  re-arms the enqueue. Distinct from the Ask-policy suspension (which auto-resumes on a settled
  `agent_decision`); a user-pause has no decision, so only an explicit `resume_run` revives it.
- **Resume re-`create`s the enqueue job** (an upsert → fresh `Running`) rather than mutating it —
  `lb_jobs::update` is `pub(crate)`, and re-create preserves the poster's identity/caps payload
  verbatim (co-trust). The drive's own `answer_already_posted` guard keeps it idempotent.
- **Stop posts `run stopped`** (a distinct, honest `agent_error`) so the dock shows a terminal
  stopped state rather than a silent gap or a misleading answer.
- **One `agent.control` cap for all three** (not three caps) — they are one control surface; the
  workspace wall + the run's own job row are the isolation.

### Tests (all green, real infra, rule 9)
- Rust host `run_control_test.rs` **6/6** — pause→Suspended + resume→Running + enqueue re-armed;
  stop→Cancelled (+ resume of a cancelled run is a bad-state error); the LOOP honors a pause
  (a pre-suspended run drives to Suspended WITHOUT completing, then resume finishes it and posts
  the answer); the worker posts `run stopped` for a cancelled run; MANDATORY cap-deny (opaque
  `Denied`, status untouched); MANDATORY ws-isolation (a ws-B principal can't touch a ws-A run).
- Rust gateway `run_control_route_test.rs` **5/5** — pause→204→Suspended + resume→204→Running;
  stop→204→Cancelled; no-cap→opaque 403 (status untouched); unknown op→400; ws-B token can't
  control a ws-A run (client error, ws-A run untouched).
- UI `DockRunStatus.test.tsx` **5/5** — Pause+Stop render & fire while working; Resume renders &
  fires when paused; controls show in the pre-delta Sent state (the run may already be driving);
  controls hidden with no handlers; Retry in the error state. UI gateway `AgentDock.gateway.test.tsx`
  gains "surfaces the pause + stop run controls while a run is in flight" (**9/9** total).
- Regression: existing `channel_agent_worker_test` **4/4** + `agent_watch_test` **8/8** stay green
  (the worker's new lifecycle classification didn't disturb the happy/deny/idempotency paths).
  `cargo build --workspace` + `cargo fmt` clean.

### Why jsdom limits the UI test
jsdom has no EventSource, so a live run's phase never leaves "sent" in a UI test and a live drive
can't be sustained — the durable pause→resume→complete path is therefore proven in Rust (host +
gateway-route), and the UI test asserts the controls surface while a run is active. Same split as
the streaming states.
