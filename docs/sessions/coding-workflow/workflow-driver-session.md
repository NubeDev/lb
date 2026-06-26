# Coding workflow — the background driver + node wiring (session)

- Date: 2026-06-27
- Scope: ../../scope/coding-workflow/workflow-driver-scope.md
- Stage: S7 — platform maturity (STAGES.md). Turns the close-the-loop verbs into a running service and
  mounts them (+ the webhook ingress) in the `node` binary. The S7 exit gate was already MET.
- Status: done

## Goal

The reactor (`react_to_approvals`) and the relay (`relay_outbox`) were proven in tests, but **nothing
ran them in a process** — `node/src/main.rs` still ended at the S1 hello demo. Ship the **background
driver** that ticks both verbs per workspace, and the **env-gated wiring** that mounts it + the webhook
front door in the binary. Make the loop a thing a node *runs*, not just a thing the tests prove.

## What changed

**New role crate `lb-role-github-workflow`** (no network deps — pure orchestration loop):

- `binding.rs`: `WorkflowBinding { ws, principal, channel }` — one workspace the driver services. The
  loop takes a *list*, so isolation is structural (each call selects its binding's `ws`).
- `drive.rs`:
  - `drive_once(node, bindings, target, now, on_error) -> Tick` — **one tick**: for each binding, a
    **reactor pass** then a **relay pass** (reactor first → a freshly-approved job's PR ships the same
    tick). A per-binding error goes to `on_error` and is skipped; the tick services the rest and the
    next tick re-reads the durable set (never lost). Returns a `Tick { started, delivered, failed,
    dead_lettered }` tally.
  - `run_workflow_loop(node, bindings, target, interval, clock, on_error)` — ticks forever, `clock()`
    supplying `now` (the binary passes wall-clock; a test passes a counter — no wall-clock in the crate).
- `lib.rs`: exports `WorkflowBinding`, `drive_once`, `run_workflow_loop`, `Tick`.

**Node binary wiring (`node/src/github.rs`, new; mounted from `main.rs`):**

- `github::mount(Arc<Node>)` — env-gated, config not code-branch (§3.1):
  - `LB_WORKFLOW_WS` is the switch (absent → solo node, the existing demo).
  - `LB_WEBHOOK_ADDR` + `LB_WEBHOOK_SECRET` → spawn the webhook front door (`serve_tenants`, one
    `default` tenant for the single-workspace deployment).
  - `LB_GITHUB_API` (+ `LB_GITHUB_TOKEN`, `LB_WORKFLOW_TICK_SECS`) → spawn the driver loop with the
    real `lb-role-github-target` `GithubTarget`.
  - `now` enters here as `unix_seconds()` — the binary is the legitimate wall-clock boundary.
- `main.rs`: `node` is now `Arc<Node>` (shared with the background tasks); `github::mount(node.clone())`
  is called after the hello demo, before the optional gateway. The demo + gateway paths are unchanged.

The three github role crates (`-webhook`, `-target`, `-workflow`) are now `node` dependencies, mounted
by config. No core crate became role-aware; the wiring is the thin binary layer §3.1 permits.

## How it fits the core (the platform checklist)

- **Symmetric nodes.** One binary; the driver + ingress are mounted by env, never an `if cloud`. A
  node with no `LB_WORKFLOW_WS` is the same binary running solo.
- **Workspace is the hard wall.** The loop services a list of bindings, each selecting its own `ws`.
  Tested: a tick over only ws-A leaves ws-B's approved job + effects untouched.
- **Capability-first.** The driver acts as each binding's service principal; the reactor re-checks
  `mcp:workflow.start_job:call`, so the deny path holds (inherited from the reactor's own test).
- **Stateless service.** The driver holds no state; a tick that errors on one ws skips it and the next
  tick re-reads the durable set (the source of truth). Kill the process and restart — nothing lost.
- **Roles depend on host, never the reverse.** The driver crate depends on `lb-host` (the verbs +
  the `Target` trait); the concrete GitHub HTTP client is `lb-role-github-target`, supplied behind the
  trait. No network dep in the driver, none in core.
- **No SDK/WIT/cap-grammar change.** A loop over existing verbs + binary wiring.

## Tests (all green — pasted below)

`role/github-workflow/tests/driver_test.rs` (4), real embedded SurrealDB + in-proc Zenoh, a recording
`Target` as the only stub:

- **one tick starts the job AND delivers the PR** (the headline — the loop closes in a single tick);
- **a second tick is a no-op** (loop-level idempotency — one job, one PR);
- **a tick over one workspace never touches another** (mandatory workspace-isolation);
- **the injected clock advances each tick** (no wall-clock in the crate; `now` threaded to both verbs).

```
$ cargo test -p lb-role-github-workflow
running 4 tests
test one_tick_starts_the_job_and_delivers_the_pr ... ok
test a_second_tick_is_a_no_op ... ok
test a_tick_over_one_workspace_never_touches_another ... ok
test the_injected_clock_advances_each_tick ... ok
test result: ok. 4 passed; 0 failed

$ cargo build --workspace          # green (incl. the `node` binary with the new wiring)
$ cargo fmt --all --check          # clean
$ cargo clippy -p lb-role-github-workflow --tests   # no warnings in the new crate
$ bash scripts/check-file-size.sh
FILE-LAYOUT: all source files within 400 lines (333 checked)
```

Net: **~214 Rust + 26 Vitest + 2 shell** tests green (+4 Rust this slice).

## Decisions & alternatives rejected

- **The loop lives in a role crate, not `lb-host`.** Putting a timer + a concrete target in core would
  make "run the loop" non-optional and pull network deps inward. The host owns the verbs; the role
  owns the cadence — the gateway/webhook split, applied again.
- **Injected clock, not `SystemTime::now()` in the crate.** Keeps the crate deterministic + testable;
  the binary (`node/src/github.rs`) is the one place wall-clock enters (testing §3).
- **A list of bindings, not a global `ws`.** Makes isolation structural and lets one process drive
  several workspaces — and the isolation test sharp (drive only A, assert B untouched).
- **Reactor before relay within a tick.** Optimization (same-tick PR), not correctness — both passes
  are idempotent. Chosen for lower latency; the inverse would just defer a PR by one tick.

## Cross-links

- Scope: ../../scope/coding-workflow/workflow-driver-scope.md (open questions: dynamic workspace set,
  LIVE-query driver, multi-driver contention, tick budget).
- Public: ../../public/coding-workflow/coding-workflow.md (the loop now has a runner).
- Drives: ./close-the-loop-session.md (the reactor + enriched payload),
  ./outbox-egress-session.md (the relay + the real `Target`),
  ../extensions/github-webhook-multitenant-session.md (the ingress mounted alongside).
- No debugging entry — nothing broke (the one compile slip, a missing `iat`/`exp`, was caught at build).
