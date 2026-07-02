# Channels in-channel agent — wall-time supervision (session)

- Date: 2026-07-01
- Scope: ../../scope/external-agent/run-lifecycle-scope.md (#5 supervision) · ../../scope/channels/channels-agent-scope.md
- Builds on: ./channels-agent-background-session.md (the durable-detached-run half of #5)
- Stage: post-S10 (channels surface; the supervision half of run-lifecycle #5)
- Status: done (a detached channel agent run is now bounded by a wall-clock ceiling; a hung/looping run
  is reaped and posts an honest `agent_error` instead of a card that spins forever)

## Goal

The background half of run-lifecycle #5 made the in-channel agent run **durable + detached** — but not
**bounded**. A run that hangs (an external subprocess spinning, an in-house loop that never settles)
would leave its enqueue job `Running` and its channel card spinning forever, with no reaper. This slice
closes that: a wall-clock ceiling around the drive, so a run that exceeds it is aborted, its subprocess
torn down, and an honest `agent_error` posted.

Exit gate: a scripted run that never settles is reaped at the ceiling and posts a `kind:"agent_error"`
carrying a distinct (non-opaque) timeout message; the enqueue job is retired (terminal), so a re-drain
is a pure no-op — no re-run, no second error.

## Approach — a `tokio::time::timeout` around the whole run future, at the drive seam

The ceiling lives in `channel::agent_worker::drive_run` (host side), wrapping the single
`invoke_via_runtime` future — **not** inside the `AgentRuntime` trait. Rationale:

- **One seam bounds every runtime.** The in-house loop and every external `AcpRuntime` reach the run
  through `invoke_via_runtime`; wrapping *that* future bounds them all uniformly, so no runtime impl has
  to re-implement a ceiling (and a future runtime can't forget to). The in-house loop already self-bounds
  via `MAX_STEPS`; this adds the wall-clock bound the *external subprocess* lacked.
- **Drop is the reaper.** On timeout, `tokio::time::timeout` drops the run future. For the external
  `AcpRuntime`, dropping the driver future closes the ACP session and the subprocess's stdio handles, so
  the child is reaped rather than left a zombie pinning the job — Drop is the teardown seam, no separate
  kill syscall needed at this layer. (An explicit ACP `session/cancel` for a *user-requested* mid-run
  stop remains open; this slice reaps a *hung* run.)
- **Fail-closed terminal outcome.** The ceiling is host authority and overrides whatever the run would
  eventually have reported — a run that blows the budget is `agent_error`, never a late success. This is
  the untrusted-agent posture the scope calls for: process/ceiling is authoritative, not the agent's
  self-reported word.

Rejected: a per-runtime ceiling inside each `run` impl (duplicated, forgettable, and the in-house impl
would need it bolted on beside `MAX_STEPS`). Rejected: a paused virtual clock in the test — the real run
does real embedded-SurrealDB I/O that a paused clock doesn't advance, so a real short ceiling against a
runtime that sleeps past it is the honest, deterministic test (reaps in ~50 ms real time).

## What changed

### `crates/host/src/channel/agent_worker.rs`
- New `RUN_WALL_CEILING` (`pub(crate)`, 15 min) — the fixed node-default ceiling (per-workspace policy is
  the deferred open question) — and `TIMEOUT_MESSAGE` ("agent run exceeded its time limit and was
  stopped"), an **honest** message distinct from the opaque `OPAQUE_DENY` (a timeout is a reportable run
  fault, not an authorization signal — it must not masquerade as a deny).
- `drive_queued_run` and `drive_run` take a `ceiling: Duration`. `drive_run` wraps the
  `invoke_via_runtime` future in `tokio::time::timeout(ceiling, run)`: `Ok(result)` maps the run's own
  `AgentError` as before (deny → opaque, else honest); `Err(_elapsed)` → `TIMEOUT_MESSAGE`.

### `crates/host/src/agent_reactor.rs`
- The production tick (`drain_spawning`) and the no-arg `drain_channel_agent_runs` pass `RUN_WALL_CEILING`.
- New `drain_channel_agent_runs_with_ceiling(node, ws, ceiling)` — the test seam that reaps at a tiny
  wall-time. Exported from `lib.rs` beside `drain_channel_agent_runs`.

### `crates/host/src/channel/mod.rs`, `crates/host/src/lib.rs`
- Re-export `RUN_WALL_CEILING` (crate-internal) and `drain_channel_agent_runs_with_ceiling` (public).

## Tests (all green)

`cargo test -p lb-host --test channel_agent_worker_test` — **7 tests**, adding:
- `a_run_that_exceeds_the_supervision_ceiling_is_reaped_with_an_agent_error`: a `HungRuntime` (its `run`
  future sleeps 3600 s) registered as `default`; the drive with a 50 ms ceiling reaps it and posts a
  `kind:"agent_error"` whose `error` is exactly the timeout message and is **not** "agent not permitted"
  (a timeout must not look like a capability deny). A re-drain posts no second error — the enqueue job
  was retired terminal and the `a:<job>` idempotency guard short-circuits.

The prior 6 (background-spawn, happy-path, idempotency, opaque deny, opaque unknown-runtime, re-entrancy,
ws-isolation) still pass unchanged (they now pass `RUN_WALL_CEILING` via the no-arg drain).

Also green: `cargo test -p lb-host --lib channel::` (27), `cargo build -p node`, `cargo fmt`.

## Mandatory categories (testing-scope)
- **Supervision:** the new hung-run reap test (above).
- **Capability-deny / workspace-isolation:** unchanged and still asserted by the existing 6 (the timeout
  path is explicitly asserted *distinct* from the opaque deny, so supervision doesn't blur the deny wall).

## Follow-ups (open, tracked in the scope)
- **ACP `session/cancel`** for a user-requested mid-run stop (this slice reaps only a *hung* run).
- **Iteration/token ceiling** for the external subprocess distinct from wall-time (in-house has `MAX_STEPS`).
- **Per-workspace ceiling policy** (fixed node default today).
- The next big #5 items: **resume** contract for the foreign loop and the **`agent.runtimes`** read surface.
