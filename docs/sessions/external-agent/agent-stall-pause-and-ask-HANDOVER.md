# Handover — external-agent stall → pause-and-ask (verify + close out)

**For:** the next session. **State:** code complete, all unit/integration tests green, full
`cargo build --workspace` green, `tsc --noEmit` clean. **What's left:** a live E2E on a running dev
node + promote docs. No code is expected to be written unless the E2E surfaces a bug.

## TL;DR of what shipped

When an external agent run makes **no progress for 90s** (`NO_PROGRESS_CEILING`), it is no longer
failed — it is **suspended (resumable)** and the dock shows an actionable **"Keep going" / "Stop"**
prompt. "Keep going" = the shipped `resume_run` (rehydrate from the cursor); "Stop" = `stop_run`
(cancel). Reuses the shipped suspend/resume/stop rails + the durable-channel-item path; no new SSE
event or control verb.

Root incident that started this: an `extension-builder` run flailed in bash (`make dev`, `cargo
build`) making zero MCP calls and burned the 15-min wall. Root cause was the MCP shim not being on
PATH (fixed: `register()` resolves `lb-mcp-shim` beside the node binary). Full story:
`docs/debugging/external-agent/agent-flails-in-shell-then-stalls.md`.

## Files changed (all committed to the working tree, not yet a PR)

**Backend**
- `rust/crates/host/src/agent/error.rs` — new non-terminal `AgentError::Stalled`.
- `rust/role/external-agent/src/lib.rs` — the no-progress watchdog: on stall, `lb_jobs::suspend` the
  run + return `AgentError::Stalled` (was `complete(Failed)`). Also `resolve_shim_bin()` +
  `with_no_progress_ceiling`/`from_resolved` test seams (from the prior session).
- `rust/crates/host/src/channel/payload.rs` — `KIND_AGENT_STALLED`, `AgentStalledPayload`,
  `ItemPayload::AgentStalled`, `agent_stalled_body`, added to the `parse_payload` allowlist.
- `rust/crates/host/src/channel/agent_worker.rs` — internal `DriveFault {Message, Stalled}`;
  `drive_queued_run` posts a `kind:"agent_stalled"` item on `Stalled` and retires the enqueue job
  (run stays `Suspended`). `STALL_PROMPT` text.

**Frontend**
- `ui/src/lib/channel/payload.types.ts` — `AgentStalledPayload` + union + `KINDS`.
- `ui/src/features/agent-dock/pendingRun.ts` — folds `agent_stalled` → `stalled`/`stalledText`.
- `ui/src/features/agent-dock/AgentDock.tsx` — `active` excludes a stalled run; passes
  `stalled`/`stalledText` + `onKeepGoing`(resume)/`onStopStalled`(stop).
- `ui/src/features/agent-dock/DockRunStatus.tsx` — the amber pause-and-ask card (Keep going / Stop).
- `ui/src/features/channel/AgentCard.tsx` + `MessageItem.tsx` — read-only "paused — open the dock"
  notice for the new kind on the channel message-list surface.

**Docs**
- `docs/sessions/external-agent/agent-stall-pause-and-ask-session.md` (this session's log).
- `docs/sessions/external-agent/agent-run-hardening-session.md` (prior session; follow-up marked done).
- `docs/debugging/external-agent/agent-flails-in-shell-then-stalls.md` (+ README index row) updated.

## Step 1 — re-run the automated suite (should be green as-is)

```bash
cd /home/user/code/rust/lb/rust
cargo build --workspace
cargo test -p lb-role-external-agent --test no_progress_test        # 1 passed (Suspended + Stalled)
cargo test -p lb-host --test channel_agent_worker_test              # 15 passed (incl. a_stalled_run_...)
cargo test -p lb-host --lib channel::payload                        # incl. agent_stalled_body round-trip
cargo fmt --check
```

```bash
cd /home/user/code/rust/lb/ui
pnpm exec tsc --noEmit                                              # 0 errors
pnpm exec vitest run src/features/agent-dock src/features/channel/AgentCard   # ~69 passed
```

Known-noise, DO NOT chase (red on clean master, unrelated):
- `SystemView.gateway`, `sqlSource.gateway`, `agent_routed_test`.
- 4 pre-existing `no-restricted-syntax` (raw `<button>`) lint errors in `DockRunStatus.tsx` — predate
  this change; my additions use `<Button>`. Leave them or fix in a separate cleanup.

## Step 2 — LIVE E2E (the actual remaining work)

Goal: prove the stall→pause→(keep going | stop) loop end to end on a real node, both buttons.

```bash
cd /home/user/code/rust/lb
make docker-build-image        # once, if not already built (container devkit builder)
make build-wasm                # before any backend test that loads wasm exts
make dev EXTAGENT=1 DEVKIT_BUILDER=container
```

Dev-login is member-shaped but carries admin-tier caps (fine for this). Open the app, open the agent
dock (workspace `acme`, persona `builtin.extension-builder`).

**Fastest way to force a stall without waiting 90s or finding a truly-stuck model:** temporarily drop
`NO_PROGRESS_CEILING` in `rust/role/external-agent/src/lib.rs` to e.g. `Duration::from_secs(8)`,
`make kill && make dev EXTAGENT=1 …` (the node does NOT hot-reload Rust — you MUST rebuild/restart;
see memory `flows-dev-node-no-hot-reload`). Then give the agent a goal that makes it think without
emitting events for a bit, or point the profile's binary at a wrapper that goes quiet. Revert the
constant before committing.

Verify, in order:
1. **Stall surfaces:** after the ceiling, the dock strip shows the amber card
   ("…may be stuck. Keep going, or stop?") with two buttons — NOT a spinner, NOT a red error.
2. **Run is durably suspended:** the run job is `Suspended` (check via the store / `agent.watch`), and
   a durable `kind:"agent_stalled"` item exists in the channel (id `a:<job>`). Reload the tab — the
   prompt is still there (it's durable, not optimistic client state).
3. **"Keep going" works:** click it → `resume_run` fires (`POST /runs/{job}/resume`), the run
   re-enters the reactor, rehydrates from the cursor, and continues (new events stream; eventually a
   real `agent_result` or `agent_error` replaces the stall — the prompt clears).
4. **"Stop" works:** on a fresh stall, click Stop → `stop_run` (`POST /runs/{job}/cancel`), the run
   goes `Cancelled`, the dock shows the terminal "run stopped" state.
5. **Isolation sanity:** a ws-B principal cannot resume/stop the ws-A run (opaque 403) — the control
   verbs are workspace-first; this is already covered structurally but worth an eyeball.

Record the run id + screenshots (light/dark) in
`docs/sessions/external-agent/agent-stall-pause-and-ask-session.md` under a new "## E2E (live)" heading.

## Step 3 — promote + commit (only AFTER the E2E passes)

- If anything broke in the E2E, log it in `docs/debugging/external-agent/` with a regression test, per
  `debugging-scope.md`.
- Update `docs/STATUS.md` if it tracks the run-lifecycle slice.
- The topic isn't on `master` yet as a PR. Branch off master, commit the working tree (Rust + UI +
  docs), open a PR. Commit trailer + PR body per the repo conventions.

## Design context (why it's built this way) — read if you need to change it

- The platform already had a full durable **suspend → decide → resume** machine and a shipped
  run-control surface (`pause_run`/`resume_run`/`stop_run`, `POST /runs/{job}/{op}`, dock
  `pauseRun`/`resumeRun`/`stopRun`). A stall is just another reason to suspend, so it reuses those —
  no new persistence, verb, or SSE event.
- A stalled run is posted as a durable `kind:"agent_stalled"` item (under `a:<job>`, like
  `agent_result`/`agent_error`), so the dock reconciles it through the existing `pendingRun.ts` path
  and it survives a reload. Chosen over teaching the client SSE `RunEvent` a `suspended` variant
  (simpler, durable-for-free).
- Rejected auto-stop-after-grace: the user chose plain suspend+ask; a paused run is cheap (subprocess
  already reaped). Easy to add later if unattended paused runs pile up.

## Open follow-ups (not blocking)

- Optional auto-stop for an unattended paused run.
- An **iteration/token** ceiling distinct from wall-time (the other half of the
  `run-lifecycle-scope.md` open item; this work did the no-progress half).
- Give the channel (non-dock) `AgentCard` its own keep-going/stop wiring if agent runs become common
  outside the dock (today it's a read-only "open the dock" notice).
```
