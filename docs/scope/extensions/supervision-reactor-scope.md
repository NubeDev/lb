# Supervision reactor scope — proactive health-poll + auto-restart for native sidecars

Status: scope (the ask). Promotes to `public/extensions/` once shipped. The follow-up that closes
two named open questions in [`native-tier-scope.md`](native-tier-scope.md) — **"Background
health-poll reactor"** and **"Boot reconciler"** (re-spawn `lifecycle=started` sidecars). It turns
the native tier from **reactively** supervised (a crash is repaired *on the next call*) into
**proactively** supervised (a crash or a hang is detected and repaired *on its own*, between calls).

> Read with: [`native-tier-scope.md`](native-tier-scope.md) (the tier this completes — its Open
> questions are this slice's ask), `README.md` §6.3 (two tiers), §6.8 (a node is the single
> authority for its own sidecars), `bus/bus-scope.md` (where supervision events go — motion, not
> truth), `observability/` (the stats/event surface this feeds). The supervisor primitives
> (`Sidecar::health`, `restart`, `next_backoff`, `Spec.health_interval`) already exist in
> `lb-supervisor` — this slice **wires** them; it does not invent them.

## The gap (why this slice exists)

The native tier ships supervision **primitives** but never connected the loop that drives them.
Concretely, today:

- `Sidecar::health()` exists ([`sidecar.rs`](../../../rust/crates/supervisor/src/sidecar.rs)) but
  is called **only from a test** — no production code polls it.
- `Spec.health_interval` (200ms default) and `Backoff` exist but **nothing reads them at runtime**;
  `next_backoff()` is dead code in production.
- `restart()` is invoked **only on-demand**: from `call_sidecar` when a `call` hits a transport
  fault ([`native/tool.rs`](../../../rust/crates/host/src/native/tool.rs)), and from the operator
  `native.restart` verb ([`native/lifecycle.rs`](../../../rust/crates/host/src/native/lifecycle.rs)).
- There is **no boot reconciler** for native: `ext/boot_load.rs` re-loads enabled **wasm** instances
  at boot, but a `lifecycle=started` native sidecar is **not** re-spawned (the native-tier scope
  deferred this — "single-process lifetime this slice; records ready").

What this means in practice (all confirmed against the code, not assumed):

1. **A crash is bounded** — a separate OS process can never bring the host down; a dead child
   surfaces as a caught transport error, not a panic. **This is solid and stays true.** This slice
   does **not** change crash-safety; it changes *when the crash is noticed*.
2. **A crash between calls is noticed lazily.** A long-running daemon (e.g. the MQTT-bridge class)
   that dies with no subsequent `call` stays dead silently until someone calls it. `ext.list` shows
   `running: false`, but nothing revives it.
3. **A hung-but-alive child is not detected at all.** A child stuck in a deadlock/infinite-loop is
   still a live PID, so `is_running` reports `true`, and the next `call` blocks on a **framed read
   with no timeout** ([`sidecar.rs` `request`](../../../rust/crates/supervisor/src/sidecar.rs)). The
   `health_interval` that exists precisely to catch this is never used.
4. **Backoff spacing is not enforced on the call path.** `restart()` does not sleep; its doc says
   "the caller applies the backoff delay," but `call_sidecar` restarts-then-retries immediately.
   `max_restarts` still caps a crash loop (so it cannot busy-spin unboundedly), but the *spacing*
   between restarts is skipped.

The sibling design wants this loop explicitly: the rubix-cube extension-server scope (`supervisor/*`)
states *"supervisor periodically sends `health` (timeout → restart)."* lb built the parts and never
ran the loop.

## Goals

1. **Proactive detection.** A background reactor health-polls each running sidecar on its
   `health_interval`; a missed reply within the window is treated as a fault.
2. **Auto-restart with real backoff.** On a detected fault (crash *or* hang), apply the
   `RestartPolicy` + `Backoff` — the **spacing is honored** (`next_backoff()` is now live), and the
   bounded `max_restarts` ceiling caps a crash loop. Past the ceiling the sidecar is left dead and a
   terminal event is surfaced (no infinite respawn).
3. **A call read-timeout** so a hung child cannot wedge a caller indefinitely — a `call` that
   exceeds the bound returns a transport fault (which the existing on-demand path already handles)
   instead of blocking forever.
4. **Boot reconciler for native.** On node boot, re-spawn every `lifecycle=started` native sidecar
   from its durable `Install` + `native_status` records — the native peer of `ext/boot_load.rs`'s
   wasm loader, so a node restart restores the supervised fleet (not just wasm).
5. **Observable supervision.** Each transition (health-ok / health-miss / crashed / restarting /
   restart-exhausted / reconciled-on-boot) is a fire-and-forget **event** on the bus (motion) and
   bumps the durable `native_status` projection (truth) — feeding the observability surface.

## Non-goals (this slice — deferred)

- **No new capability grammar.** Same posture as native-tier: the reactor is host-internal
  machinery driven by the node it runs on; it adds no `process:` surface and no new MCP verb. (It
  *reads* the same records the gated `native.*`/`ext.*` verbs write.)
- **No OS-level hardening** (cgroups/seccomp/userns) — still the native-tier non-goal. This slice is
  about *liveness*, not deeper *sandboxing*.
- **No cross-replica supervision.** A live child is per-node local state (§6.8). The reactor
  supervises **its own node's** sidecars only; it does not coordinate a child across replicas.
- **No wasm health reactor here.** Wasm liveness (fuel/epoch/memory limits + a per-extension circuit
  breaker) is its own deferred slice — see [`core/`](../core/) "fuel/epoch not yet tuned." This
  slice is native-only; the two share the *concept* (a breaker), not the mechanism.
- **No change to crash-safety.** Process isolation + caught transport faults + bounded restart are
  already correct and untouched. This slice is strictly additive liveness.

## Intent / approach

**One reactor task per node, owning the supervision loop — a host service beside `native`, not a
change to the `Sidecar` itself.** The `Sidecar` stays a passive handle (health/call/restart
primitives); the reactor is the active driver. This keeps the FILE-LAYOUT blast-radius small: a
`Sidecar` is "one supervised child's control channel," the reactor is "the policy that decides when
to poll and restart" — distinct responsibilities, distinct files.

- **The reactor is a `tokio::spawn`'d task** started at node boot (after the boot reconciler
  re-spawns the fleet). It walks the runtime `SidecarMap`, and for each `(ws, ext_id)` whose
  `health_interval` has elapsed, sends a `health` request. A timeout/transport fault → apply the
  restart policy with `next_backoff()` spacing, bump `restart_count` in `native_status`, emit the
  event. This is the **same restart code path** `call_sidecar` already uses (`Sidecar::restart` +
  `bump_restart_count`) — reused, not forked, so on-demand and proactive restart converge.
- **Stateless invariant unchanged.** The reactor holds no durable state; it reads the runtime map
  (cache) and the records (truth). A respawn re-derives everything from the `Install` spec exactly
  as the on-demand path does. The PID is disposable; the record is authority.
- **Boot reconciler is the native peer of `boot_load.rs`.** On boot: list installs, for each
  `tier=Native ∧ enabled ∧ native_status.lifecycle=started`, re-spawn via the `Launcher` and
  register the handle — then start the reactor over the restored map. Same shape as the wasm loader;
  same `LoadedExt`-style log return for boot diagnostics.
- **The call read-timeout** is a bound on `Sidecar::request`'s framed read (configurable on the
  `Spec`, defaulting off-the-`health_interval` × a factor, or an explicit `call_timeout`). On expiry
  it returns `SupervisorError::Transport("call timed out")` — which `call_sidecar` already treats as
  "child died → restart-and-retry," so a hung child is repaired by the existing path the moment it's
  called, and by the reactor before that.

**Alternative considered & rejected — per-sidecar watcher tasks (one task per child).** Tempting
(each `Sidecar::spawn` also spawns its own watcher), but it scatters lifecycle across N tasks that
must each be cancelled exactly on stop/restart/uninstall (a zombie-task hazard the native-tier scope
already flags for processes). One reactor sweeping the map has a single, auditable cancellation point
(node shutdown) and no task-leak-on-uninstall surface. Rejected for the per-task isolation it buys
at the cost of N cancellation edges; revisit only if a single sweep can't keep up with the fleet size
(it can — health is low-rate request/reply).

**Alternative considered & rejected — drive the reactor off bus events instead of a poll.** A child
could heartbeat *to* the host. But that trusts a buggy/hung child to report its own unhealth — the
exact case we need to catch. A host-driven poll detects a child that has stopped cooperating;
self-report cannot. The poll is the correct direction; the bus carries *our* observations *out*, not
the child's claims *in*.

## How it fits the core

- **Tenancy / isolation:** the reactor walks the `SidecarMap` keyed `(ws, ext_id)`; every restart
  re-mints the **same workspace-scoped token** the install computed, so a respawned child is bounded
  identically. The reactor never crosses workspaces — it operates per-entry, and `native_status`
  writes stay workspace-namespaced. Mandatory isolation test: a ws-A reactor restart writes only
  ws-A's `native_status`; ws-B sees nothing.
- **Capabilities:** the reactor adds **no** new gated verb (host-internal). The records it reads
  were written by the already-gated `native.install`/`ext.*` verbs; nothing reachable by an external
  caller changes. Mandatory deny test stays the native-tier one (no `mcp:native.install:call` → no
  child → nothing for the reactor to supervise).
- **Data (SurrealDB):** reuses `native_status:{ext_id}` (lifecycle, restart_count, last_ts) — the
  reactor bumps `restart_count` and stamps `last_ts`; **no new table.** A `restart-exhausted`
  terminal state is recorded so `ext.list`/`native.status` report a dead-and-not-coming-back sidecar
  distinctly from `stopped` (operator intent) — additive to the existing `Lifecycle` enum.
- **Bus (Zenoh):** supervision transitions are fire-and-forget motion to an ops channel (§6.2) —
  signals for live observability, **not** truth. A missed event loses nothing (the record is
  authority). No outbox effect (these are observations, not must-deliver work).
- **Sync / authority:** §6.8 — a node is the sole authority for its own sidecars, so the reactor is
  inherently node-local; no cross-node coordination, no claim of multi-replica supervision.
- **Symmetric nodes (§3.1):** the reactor is the same code on edge and cloud — it reads the same
  records and runs the same loop. No `if cloud`. A node with no native installs simply has an
  empty map and an idle reactor.

## Example flow

The proactive-restart proof (the new exit-gate path), end to end:

1. A `lifecycle=started` native sidecar `mqtt-bridge` is running in workspace `acme`; the reactor
   has it in the `SidecarMap` and polls `health` every `health_interval`.
2. **The child hangs** (deadlocks — PID alive, stops replying). The next reactor poll's `health`
   request **times out**. The reactor treats the timeout as a fault: emits `native:health-miss`,
   applies `RestartPolicy::OnCrash` with `next_backoff()` spacing (so a flapping child is spaced,
   not busy-respawned), kills the process group, and **respawns** from the durable `Install` record —
   re-minting the scoped token. `restart_count` bumps in `native_status`; **no other durable state
   changes**.
3. After the respawn the reactor's next `health` succeeds (`native:health-ok`); the sidecar answers
   calls again — **with no caller having had to trip the fault first.** (Contrast today: the hang
   would sit undetected until a `call` blocked on it.)
4. If the child crash-loops, the reactor stops respawning at `max_restarts`, records
   `lifecycle=restart-exhausted`, and emits a terminal `native:restart-exhausted` event. `ext.list`
   now shows the sidecar dead-and-stopped-trying (distinct from operator `stopped`); an admin
   `native.restart` (cooperative) is the deliberate way back.
5. **Node restart:** the boot reconciler lists installs, finds `mqtt-bridge` is
   `enabled ∧ started`, re-spawns it from records, registers the handle, and the reactor resumes
   polling — the supervised fleet is restored across a node restart (today only wasm is).

## Testing plan

Mandatory categories (testing-scope §2) plus the proactive-supervision categories:

- **Capability-deny** (mandatory, unchanged) — `denies_install_without_grant` still holds; the
  reactor adds no reachable verb, so there is nothing new to deny. Assert the reactor never
  supervises a sidecar that was never gated-installed.
- **Workspace-isolation** (mandatory) — `reactor_restart_writes_only_owning_ws_status`: a forced
  fault on a ws-A sidecar bumps only ws-A `native_status`; a ws-B `native.status` is unaffected and
  ws-B reading the store sees no ws-A change.
- **Proactive crash** — `crashed_between_calls_is_restarted_by_reactor`: spawn a **real** child,
  kill the OS process, make **no** call, and assert the reactor respawns it (poll for eventual
  `restart_count` increment + a successful subsequent health) — proving repair *without* a triggering
  call (the gap this slice closes).
- **Hang detection** — `hung_child_is_health_timed_out_and_restarted`: a fake child that accepts
  `init` then **never replies** to `health`; assert the reactor times out within the window and
  restarts it, and that a concurrent `call` returns a transport fault (not an indefinite block) via
  the read-timeout.
- **Backoff spacing** — `reactor_spaces_restarts_by_backoff`: a crash-looping child is respawned at
  increasing `next_backoff()` intervals (assert ordering/bounds with injected logical time, not a
  fixed sleep) and stops at `max_restarts` with `lifecycle=restart-exhausted` recorded.
- **Boot reconciler (native)** — `boot_respawns_started_native_sidecars`:
  install+start a native sidecar, drop the runtime map (simulate a node restart), run the reconciler,
  and assert the sidecar is re-spawned from records and answers a tool call — and that a `disabled`
  or `stopped` one is **not** respawned (honors durable intent, like the wasm path).
- **Crash-safety regression** (guards the property the user asked about) —
  `child_crash_never_panics_host`: a child that aborts/segfaults mid-supervision surfaces as a caught
  error and leaves the host task healthy (the reactor keeps sweeping the rest of the map).
- **Supervisor unit** (`lb-supervisor`): the read-timeout fires on a stalled frame; `next_backoff`
  schedule is correct; the reactor sweep visits every map entry and skips entries not yet due.

Externals mocked: a **real OS process** for the proactive-crash + crash-safety proofs (a true
external, testing §3); the `Launcher`/fake child + injected logical time for the deterministic
hang/backoff/reconciler unit paths. Real embedded SurrealDB + in-proc Zenoh elsewhere. `Node`-booting
tests use `#[tokio::test(flavor = "multi_thread", worker_threads = 1)]` and a **unique workspace id**.

## Risks & hard problems

- **The reactor must not race the operator path.** A reactor restart firing *during* an operator
  `native.stop`/`native.restart`/`ext.disable` (or an uninstall) must not resurrect a child the
  operator just killed, nor double-spawn. Mitigation: the per-`(ws,ext_id)` handle is already behind
  a `Mutex`; the reactor takes the same lock, and **checks durable intent** (`enabled ∧ started`)
  before respawning — so a disabled/stopped/uninstalled child is left dead even mid-sweep. This is
  the load-bearing correctness invariant of the slice.
- **Zombie / double-spawn on restart.** Same hazard the native-tier scope flags, now reachable from
  a *timer* not a call. Mitigation: kill-the-process-group-and-await-exit before relaunch (already in
  `Sidecar::restart`), one owned read-loop per child, and the single reactor (no competing watcher).
- **Determinism.** OS timing makes "did the reactor restart it" non-deterministic. Assert *eventual*
  restart with a bounded poll (not a fixed sleep) for real-process tests; inject logical time +
  `Launcher` fake for the backoff/hang/reconciler units.
- **A poll storm on a large fleet** is not a real risk at the health rate (low-rate request/reply,
  one sweep), but the sweep must be **non-blocking per entry** — one slow/hung child's `health` must
  not stall the others' polls. Mitigation: the per-entry health is awaited with the read-timeout, so
  a hung child bounds its own slot, not the sweep.
- **`restart-exhausted` is a new terminal state** an operator must be able to escape. The cooperative
  `native.restart` resets the count and re-arms supervision; document this so a dead sidecar isn't
  read as permanently unrecoverable.

## Open questions

- **One reactor vs. a small pool.** A single sweep is correct and simplest; if a fleet ever grows
  past what one task comfortably polls within `health_interval`, shard the map across a few reactor
  tasks. Default: **one reactor**; shard only if measured.
- **Health-miss tolerance (N strikes vs. 1).** Restart on the first missed `health`, or require N
  consecutive misses to avoid restarting over a transient GC pause / scheduler hiccup? Default:
  **a small N (e.g. 2)** with the window = `health_interval`, configurable on the `Spec`. Flip if a
  workload proves flakier or stricter.
- **Should the reactor emit on the same ops channel as the lifecycle verbs**, or a dedicated
  `supervision/*` topic? Default: reuse the native lifecycle ops channel (one place to watch a
  sidecar's story); split if observability wants supervision isolated.
- **Wasm liveness parity.** This is native-only by design; the wasm circuit-breaker + fuel/epoch
  limits are a sibling slice. Should they share a `Reactor` abstraction, or stay separate (process
  health-poll vs. in-process fuel/trap accounting are genuinely different)? Leaning **separate**;
  decide when the wasm-limits slice lands.

## Related

- [`native-tier-scope.md`](native-tier-scope.md) — the tier this completes; its Open questions
  ("Background health-poll reactor", "Boot reconciler") are this slice's ask. Resolve those entries
  to point here when this ships.
- `README.md` §6.3 (two tiers), §6.8 (single authority per node), §3.4 (stateless extensions — the
  invariant a respawn relies on).
- `bus/bus-scope.md` (supervision events as motion, not truth) · `observability/` (the surface the
  events + `native_status` feed).
- `crates/host/src/ext/boot_load.rs` (the wasm boot loader this mirrors for native) ·
  `crates/supervisor/src/sidecar.rs` (the `health`/`restart`/`next_backoff` primitives this wires).
- rubix-cube `extension-server-scope.md` (`supervisor/*`: "periodically sends `health`, timeout →
  restart" — the behavior this matches).
