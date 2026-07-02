# Channels in-channel agent — background/supervised execution (session)

- Date: 2026-07-01
- Scope: ../../scope/channels/channels-agent-scope.md · ../../scope/external-agent/run-lifecycle-scope.md (#5)
- Builds on: ../channels/channels-agent-session.md (v1 — the inline worker this detaches)
- Stage: post-S10 (channels surface; the durable-run half of run-lifecycle #5)
- Status: done (the in-channel agent run is now detached, durable, and idempotent — no longer tied to
  the POST connection)

## Goal

The channels-agent v1 drove the agent run **inline** inside `channel::post`, so the run was tied to the
held POST connection: it blocked the handler for the run's duration and **closing the tab mid-run
cancelled it** (and a node restart lost it). This slice is run-lifecycle #5's biggest-value half: make
the run **non-blocking + detached + durable** — `post` returns the instant the request lands, and a
background reactor drives the run off the connection, so it survives the tab closing and a node restart.

Exit gate: posting a `kind:"agent"` item returns from `post` **before** the run completes; the durable
`agent_result` appears only after the background reactor drains the queue; a re-drain (a second tick / a
restart mid-queue) never re-runs or double-posts.

## Approach — Option A (durable job + background reactor), the faithful #5 delivery

Considered the two options the continuation note laid out:

- **Option A (chosen): the run is a durable `lb-jobs` job driven by a background reactor** that holds
  `Arc<Node>`, mirroring the shipped `spawn_flow_reactors`. Durable (survives node restart), non-blocking
  (post returns before the run), idempotent, resumable, and it reuses a proven pattern.
- **Option B (rejected): thread `&Arc<Node>` into `post` and `tokio::spawn` the run.** Smaller, but the
  spawned task is a bare detached future — it survives the POST connection closing but is **not durable**
  (a node restart mid-run silently loses it) and is not resumable. #5 explicitly wants durable +
  supervised + resumable, so Option B would leave the exit gate half-met and require a rework later.

The insight that makes A cheap: the `agent` **request item is already durable** in the inbox, and the
**run itself is already a durable `agent-session` job** (`run_session` owns it). All that was missing was
a durable *enqueue signal* the reactor can drain — one small job kind + one small list verb.

## What changed

### `lb-jobs`: a `pending` drain-scan verb (new `crates/jobs/src/pending.rs`)

`lb-jobs` had `create`/`load`/`complete`/… but **no list**. Added `pending(store, ws, kind)` — scans the
ws-namespaced `job` table and returns jobs whose `kind` matches AND whose status is still
`is_resumable()` (so a `Done`/`Failed`/`Cancelled` job is drained, never re-driven). Bounded to one page
(`MAX_SCAN_LIMIT`); a reactor ticks repeatedly, so a backlog drains across ticks, not in one unbounded
read. Raw verb, no caps gate (like every jobs verb — the caller holds its own authority). One
responsibility per file (FILE-LAYOUT).

### `channel`: the durable enqueue record (new `crates/host/src/channel/agent_job.rs`)

`ChannelAgentJob { cid, goal, runtime?, run_job, poster_sub, poster_caps, ts }` — serialized into the
enqueue job's opaque `payload`. It carries everything the reactor needs to drive the run **exactly as
the inline worker would have**, under the **poster's** authority: `poster_sub` + `poster_caps` let the
reactor reconstruct the poster via `Principal::routed` — the SAME co-trust reconstruction the
routed-agent hub already performs (in-process, ws-scoped, unsigned; never used to widen — the run's
effective grant is still `agent ∩ poster` at every tool call). Two ids, deliberately distinct so the
two durable records never collide in the shared `job` table: enqueue job = `q:<run_job>`, run job =
`<run_job>`; the correlated answer item = `a:<run_job>` (also the idempotency key).

### `channel::agent_worker`: `run_if_agent` now ENQUEUES; `drive_queued_run` is the drained drive

- `run_if_agent` (still called from `post`, re-entrancy-guarded on `kind:"agent"`) no longer drives the
  run — it captures the poster's identity + caps onto a `ChannelAgentJob` and `lb_jobs::create`s the
  enqueue job (`q:<run_job>`, idempotent), then returns. `post` finishes the instant that persists.
- `drive_queued_run` is the work that used to run inline — unchanged in substance (opaque/honest
  error split, 256 KB answer cap, `invoke_via_runtime` under the reconstructed poster, post
  `agent_result`/`agent_error` under `system:agent-worker` as `a:<run_job>`), now called by the reactor.
  It **short-circuits idempotently**: if `a:<run_job>` already exists (the run completed on a prior tick
  or before a restart), it does not re-drive (no re-run, no re-spend, no double-post) and just retires the
  enqueue job. On completion it marks the enqueue job `Done` so the next drain skips it.

### `agent_reactor` (new `crates/host/src/agent_reactor.rs`): the background driver

Twin of `spawn_flow_reactors`. One detached task per node, ticking a ws-scoped `lb_jobs::pending` scan on
a cadence (2 s). Two entry shapes over one shared `scan_drivable`:

- **`spawn_agent_reactors`** (production): each tick **spawns** a `drive_queued_run` per pending job so a
  long run never stalls the tick or the rest of the queue. **No double-drive, two guards:** an in-process
  `in_flight` set (a tick skips a run it already spawned — the enqueue job legitimately stays `Running`
  until the drive retires it) **and** the durable `a:<run_job>`-exists idempotency check inside
  `drive_queued_run` (survives a restart / crash mid-drive). A malformed enqueue record is retired
  (`Failed`) so it stops re-appearing.
- **`drain_channel_agent_runs`** (synchronous flush): drives every pending run inline + sequentially and
  returns only when they've all posted their result and retired their job — the deterministic drain a
  **test** (or any caller wanting an immediate flush) uses without the timer.

### `node/src/main.rs`: wire the reactor at boot

`lb_host::spawn_agent_reactors(node.clone(), vec![ws], 2s)` beside the existing
`spawn_flow_reactors(…)`. One detached owner per node for the configured workspace. Placement is config,
never a code branch (rule 1); symmetric edge/cloud.

## Design decisions

- **Two jobs, two responsibilities.** The enqueue job (`channel-agent-run`, `q:<run_job>`) is the durable
  "a run is queued" signal the reactor drains; the run itself is the separate `agent-session` job
  (`<run_job>`) that `run_session` owns (transcript/cursor/resume). Keeping them distinct means the
  reactor's `pending` scan (filtered by kind) never picks up run jobs, and resume machinery is untouched.
- **Poster authority travels on the record, reconstructed via `Principal::routed`.** The reactor is a
  node-internal actor, but the *run* must act as the asker — so the poster's sub+caps ride the durable
  record and are rebuilt exactly as the routed-agent hub rebuilds a remote caller. Same unsigned
  in-process co-trust caveat, same ws-scoped wall; never widens.
- **Idempotency is `a:<run_job>`-exists, not a lock.** The answer item's presence is the durable "this
  run is done" fact; checking it makes a re-drain (tick overlap, restart mid-queue) a safe no-op without
  any distributed lock. The in-process `in_flight` set is only an optimization to avoid spawning a second
  drive while the first is still running within one process.
- **Enqueue failure is swallowed, never falls back to inline.** Driving inline on an enqueue failure would
  re-tie the run to the POST connection we are deliberately detaching; the request item already landed, and
  the reactor only ever drains what durably persisted.

## Not built here (named, linked TODO — not faked)

- **Supervision (the other half of #5):** wall-time/iteration ceiling + kill/reap of a hung run + a
  `session/cancel` path. The run is now durable and detached; bounding + reaping it is the next #5 slice.
  (The in-house loop already has `MAX_STEPS`; an external subprocess ceiling + reaper is still open.)
- **`agent.runtimes` read verb + composer runtime picker** (#5 read surface / next step #5) — unchanged.
- **External-agent #3 wall / #4 model-routing** — still the gates before any *production* external run;
  this slice changes only *where/how* the run is driven, not the external-safety posture.
- **Per-token in-house streaming** (next step #4) — unchanged.

## Tests (rule 9 — real store/bus/loop/channel/job-queue; only the model-provider HTTP ever stubbed)

- **`lb-jobs` unit-ish integration** `crates/jobs/tests/pending_test.rs` (2, real `mem://` store): `pending`
  lists only running jobs of the requested kind (a different kind + a terminal job are excluded); the scan
  is workspace-scoped (ws-B never sees a ws-A job).
- **`lb-host` unit** `channel/agent_job.rs` (3): id derivation + distinctness, opaque-payload round-trip,
  absent-runtime omitted. (Existing `payload`/`agent_worker`/`query_worker`/`chart` units unchanged, green.)
- **`lb-host` integration** `crates/host/tests/channel_agent_worker_test.rs` (6, REAL in-house loop over
  `MockProvider`, driven through the REAL reactor drain):
  - **background spawn (#5):** `post` returns with the `agent_result` ABSENT (only the request item is
    durable); the answer appears only after `drain_channel_agent_runs`.
  - **happy path:** after the drain, `agent_result` with the answer, `a:<job>` id, `runtime:"default"`.
  - **idempotency (#5):** draining twice posts exactly one result (no re-run, no double-post).
  - opaque capability-deny · opaque unknown-runtime · re-entrancy (an `agent_result` item enqueues nothing)
  - **workspace isolation:** a ws-B drain does NOT drive the ws-A run; the ws-A drain does; a ws-B reader
    sees nothing.

Commands (the `role/cli` WIP breaks `--workspace` resolution, so build the touched crates directly):
`cargo test -p lb-jobs -p lb-host -j 2` (green) · `cargo build -p node` (green — reactor wired) · `cargo fmt`.

## Follow-ups (updated)

1. **Supervision** (the rest of #5): wall-time/iteration ceiling + kill/reap of a hung/looping run,
   surfaced as an `agent_error` not a stuck card; `session/cancel` for the external subprocess.
2. External-agent #3 capability-wall before any non-dev external run.
3. `agent.runtimes` read verb → a runtime picker in the composer instead of a typed `@id`.
4. Move the in-house `run_session` onto the same per-line live-tap so its per-token deltas stream too.

## Known repo hazard (not introduced here)

An untracked, in-progress `rust/role/cli` crate is a `[workspace]` member whose `Cargo.toml` points
`[[bin]] lb` at a `src/main.rs` that does not exist, so `cargo build/test --workspace` fails at target
resolution before any of this slice's crates compile. The touched crates build + test green in isolation
(`-p lb-jobs -p lb-host -p node`); the workspace-wide command is blocked until that concurrent crate is
completed (same hazard flagged in the prior channels-agent session).
