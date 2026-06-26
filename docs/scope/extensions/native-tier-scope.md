# Native tier scope — the supervised Tier-2 sidecar

Status: scope (the ask). Promotes to `public/extensions/` once shipped. The second S7 vertical
slice and the **remaining half of the S7 exit gate**: "a native sidecar is supervised and restarts
cleanly" (`STAGES.md` S7). It is the peer of the Tier-1 wasm runtime — same control plane, same
identity/capability model, same workspace wall — for the class of extension that needs a real OS
process (a language server, an MQTT bridge, anything with its own socket/thread/long-lived daemon).

> Read with: `README.md` §6.3 (runtime: two tiers — this lands Tier 2), §6.4 (install/trust),
> `extensions/extensions-scope.md` (the manifest — this adds its `[native]` block, the deferred
> "Native (`tier="native"`) manifest fields (exec, supervision, socket) — S7"), `registry/`
> (the registry slice this composes with — a native artifact installs through the same `Source`),
> `auth-caps/auth-caps-scope.md` (the grammar — and why we do **not** change it here),
> `node-roles/node-roles-scope.md` + `platform-targets/platform-targets-scope.md` (filled in
> alongside this slice for the placement/target questions a process raises).
> The supervisor mechanics are **re-authored (not copied)** from the sibling project's design —
> `/home/user/code/rust/rubix-cube/docs/scope/extensions/extension-server-scope.md` (`supervisor/*`),
> re-based onto our primitives (`STAGES.md` "Reuse: the extension server").

The Tier-1 wasm runtime is built and proven (S1→S7). This slice adds the **escape hatch**: an
extension whose backend is an OS child process the host spawns, supervises (health-checks,
restart-on-crash with backoff, cooperative shutdown), and talks to over a framed JSON-RPC line —
while keeping the supervision **stateless** (the live PID is runtime-only; the durable truth is a
SurrealDB record), keeping it **workspace-walled** (a child carries only its workspace's scoped
identity), and keeping it behind **two independent gates** (the capability gate to spawn, the
existing signature gate if it came from the registry).

## Goals

1. **Supervise a native child end to end** — spawn it, perform the handshake, health-check it,
   stop it cooperatively, and **restart it cleanly after a crash** with bounded backoff.
2. **Prove the exit gate**: a killed sidecar is restarted and resumes answering, with **no durable
   workspace state lost** (the stateless-extension guarantee carried into Tier 2).
3. **One control plane for both tiers.** `install` / lifecycle (`start`/`stop`/`restart`) /
   `status` are the same verbs whether the backend is wasm or native — the tier is an
   implementation detail behind one surface, not a forked subsystem.
4. **One identity & isolation model.** A native child is a workspace-scoped principal carrying only
   its granted caps; install/lifecycle are capability-gated; a ws-B caller can never see or touch a
   ws-A sidecar. No new privilege path.
5. **Compose with the registry.** A `tier="native"` artifact installs through the *same* signed
   pull→verify→cache→install flow the registry slice shipped — the supervisor is what `install`
   dispatches to when the verified manifest says `native`.

## Non-goals (this slice — deferred, recorded in Open questions)

- **OS-level hardening** (cgroups resource caps, seccomp syscall filters, user-namespace / chroot
  isolation). This slice ships **process-group isolation + a scoped identity + bounded restart**;
  deeper sandboxing is a noted follow-up. (Chosen posture: "minimal proven sidecar.")
- **A real network/socket data plane to the child** beyond the control line. The child talks the
  control JSON-RPC over stdio; a child that wants to reach host capabilities does so by calling back
  through the **routed MCP namespace** with its injected scoped token (the same path an edge user
  uses) — not a bespoke socket. Wiring that callback transport to a real node is an S7 follow-up
  (mirrors the registry's "real HTTP `Source`" deferral); this slice proves supervision with a
  child that needs only the control line.
- **The federated/native UI half** (a panel rendering a sidecar's live stats). Out of scope here;
  the `native.status` MCP verb returns the stats, and a `NativeView` renders them through the fake —
  the real UI transport wiring is the same deferred gateway/Tauri work as S4/S5/S6.
- **Cross-replica supervision.** A live child is per-node local state (like the wasm instance and
  the registry cache). Single-node-per-workspace authority holds (§6.8); we do not claim a child
  supervised across replicas.
- **Changing the capability grammar.** See *Intent* — spawning is gated as a host-native MCP verb,
  **not** a new `process:` surface. A new surface is a deliberate grammar change (auth-caps) and is
  not warranted by this slice.

## Intent / approach

**A native sidecar is a host service that owns an OS child, exposed through the existing seams —
not a second extension system.** The shape mirrors the registry slice exactly (a host service
beside `agent`/`channel`/`assets`/`workflow`/`registry`, one verb per file, behind a host-owned
seam with a deterministic test impl):

- **The supervisor is a seam, like `Source`/`Target`/`ModelAccess`.** A new `lb-supervisor` crate
  owns the OS plumbing — spawn a child, frame `Content-Length` JSON-RPC over its stdio, health-poll
  it, cooperative-shutdown-then-kill, and a `RestartPolicy` + exponential `Backoff` on crash. The
  host `native` service drives it the way the `registry` service drives `Source`. **The actual
  child-spawning is behind a `Launcher` trait** so tests inject an in-process fake child (a real OS
  process for the supervision-restart proof; a fake for the deny/isolation unit paths) — the same
  "mock only the true external" rule the registry's `Source` follows (testing §3: a real process is
  a true external).

- **Supervision state is runtime-only; the durable truth is a record.** The live `Sidecar` handle
  (PID, child stdio, restart count, last-health) lives **only** in a runtime map on the `Node` (an
  `Arc<RwLock<…>>` exactly like the MCP `Registry`). The **durable** state is the existing S4
  `Install` record (now also written for `tier="native"`) plus a small `native_status` projection
  (last-known lifecycle + restart count) in the workspace namespace — so a restart re-derives
  everything it needs from the record and **loses no durable state** (§3.4). This is the
  stateless-extension rule applied to a process: the PID is disposable, the record is the truth.

- **Two gates, unchanged.** Capability gate: `install`/lifecycle are host-native MCP verbs
  `native.<verb>` gated `mcp:native.<verb>:call` (workspace-first), authorized through the same
  `authorize_tool` chokepoint as `registry.*` and `workflow.*` — no grammar change, exactly the
  proven pattern. Signature gate: a native artifact pulled from the registry is `verify_artifact`'d
  before caching, same as a wasm one — installing a native extension does not bypass the signature
  gate. **Granted ≠ trusted; trusted ≠ granted**, carried verbatim into Tier 2.

- **Identity is the injected scoped token.** The supervisor injects the child's workspace id +
  extension id + a **scoped principal token** (minted carrying exactly `granted = requested ∩
  admin_approved`, the same `lb_auth` mint the agent uses for delegation) into the child's
  environment (`LB_EXT_WS`, `LB_EXT_ID`, `LB_EXT_TOKEN`). The child calls back into the host's MCP
  namespace with that token; it can do nothing the grant doesn't allow — a compromised child is
  bounded by its scoped key + process-group isolation, never the host's credentials.

**Alternative considered & rejected — a `process:` capability surface** (`process:<id>:spawn` /
`:supervise`). The grammar has exactly four surfaces and "a new surface is a deliberate grammar
change" (`caps/request.rs`). Spawning is authority, but it is already expressible as "may call the
`native.install` host tool" — gating it as `mcp:native.install:call` reuses the proven host-service
gate with **zero grammar change** and keeps the deny/isolation tests identical to the registry's.
Adding a surface would be a forever decision (§13) bought for no capability the MCP gate can't
already express. Rejected for this slice; revisit only if native extensions ever need
finer-than-per-verb spawn authority. **Flagged loudly** per the non-negotiables: this slice touches
the OS-process boundary but **does not** touch the SDK/WIT world or the capability grammar.

**Alternative considered & rejected — supervise wasm and native through one polymorphic runtime.**
Tempting to make `Engine` itself grow a `native` backend. But the wasm instance is in-process and
synchronous-ish (a `&mut` call on a store); a child is an async OS process with stdio framing,
health timers, and restart policy — genuinely different responsibilities (FILE-LAYOUT blast-radius
test). They share the *control plane* (`install`/lifecycle/`status`) and the *identity model*, not
the runtime. So `lb-supervisor` is its own crate; the host `native` service is the one place that
dispatches "this manifest says native → supervisor; says wasm → engine."

## How it fits the core

- **Tenancy / isolation:** the child carries only its workspace's scoped token; the durable
  `Install` + `native_status` records are workspace-namespaced (structural isolation, §7); the
  runtime sidecar map is keyed by `(ws, ext_id)` so a ws-B lifecycle call can never resolve a ws-A
  child. Mandatory workspace-isolation test: ws-B sees no ws-A sidecar in the store **or** through
  the MCP `native.status` verb, and cannot stop/restart it.
- **Capabilities:** `install`/`start`/`stop`/`restart`/`status` each gated `mcp:native.<verb>:call`
  through `authorize_tool`, workspace-first. Mandatory deny test: no `mcp:native.install:call` grant
  → no spawn, no record, refused at the gate before any process starts. The child's *own* callback
  authority is its injected scoped token (`requested ∩ admin_approved`) — it can call host tools
  only within that set (re-uses the agent's delegation mint; a native child is a delegated
  principal exactly like the agent acting for a caller).
- **Placement:** native sidecars are `placement = "either"` by default but a native binary is
  **platform-specific** (unlike a portable `.wasm`) — so placement interacts with
  platform-targets (filled alongside). A `local-only`/`cloud-only` native extension is simply not
  scheduled off-role; there is **no `if cloud`** — placement is manifest metadata the loader reads
  (symmetric nodes, §3.1).
- **MCP surface:** new host-native verbs `native.install`, `native.start`, `native.stop`,
  `native.restart`, `native.status` (the lifecycle control plane). A native extension's *own* tools
  (e.g. a language-server's `format`) are registered as `Remote`-style entries in the MCP
  `Registry` so `<id>.<tool>` resolves and dispatches to the child over the control line — the same
  `Target` seam S3 added for cross-node routing, reused so the call path doesn't fork.
- **Data (SurrealDB):** the existing `install:{ext_id}` record (now written for native too) +
  a `native_status:{ext_id}` projection (lifecycle, restart_count, last_ts) in the workspace
  namespace. No new datastore. **State** only — the live PID is not state, it is motion-adjacent
  runtime that the record lets us reconstruct.
- **Bus (Zenoh):** supervision **events** (spawned / health-ok / crashed / restarted) are
  fire-and-forget motion to a channel for live observability (§6.2) — they are signals, not the
  truth. The truth is the record; a missed event loses nothing. **Must-deliver** work (if a sidecar
  later needs to emit a durable effect) goes through the **outbox**, never these events (§6.2/S6) —
  but this slice's sidecar emits none, so no outbox effect is added here.
- **Sync / authority:** the child is node-local; its records sync as ordinary `(table,id)` upserts
  on the same path the channel/asset tables use (an S7 follow-up to actually wire, like the others).
  A node is the single authority for its own sidecars (§6.8).
- **Secrets:** the injected `LB_EXT_TOKEN` is the only secret material; it is minted per-spawn
  carrying exactly the granted set and injected via env (never logged, never in a record). A child's
  config secrets (e.g. an upstream password) would be `secret:`-grant-mediated refs — out of scope
  for this slice's sidecar (it needs none).

## Example flow

The supervision-restart proof (the exit-gate path), end to end:

1. An admin calls `native.install` for `echo-sidecar` (a `tier="native"` reference extension) in
   workspace `acme`, with the manifest's `capabilities.request` admin-approved. The MCP gate passes
   (`mcp:native.install:call`).
2. The host computes `granted = requested ∩ admin_approved`, **persists the `Install` record** (the
   same S4 verb, now for a native tier), then asks the supervisor to **spawn** the child via the
   `Launcher`. The supervisor mints a scoped token, injects `LB_EXT_WS=acme` / `LB_EXT_ID=echo-sidecar`
   / `LB_EXT_TOKEN=<scoped>` into the env, and execs the binary.
3. **Handshake:** the child sends `init` (reporting it is ready); the supervisor records the
   `Sidecar` handle (PID, child stdio) in the runtime map keyed `(acme, echo-sidecar)` and writes
   `native_status = {lifecycle: started, restart_count: 0}`. A `native:spawned` event goes to the
   ops channel (motion).
4. A caller invokes the sidecar's tool `echo-sidecar.echo` through the normal MCP `call` path; it
   resolves to the `(acme, echo-sidecar)` child, dispatches one `Content-Length` JSON-RPC request
   over the control line, and returns the child's reply. (Proves the child answers.)
5. **The child is killed** (the test kills the OS process — a crash). The supervisor's health poll
   (or the read-loop EOF) detects the exit, emits `native:crashed`, applies the `RestartPolicy`
   (`OnCrash`) with `Backoff`, and **respawns** from the durable `Install` record — re-minting the
   token, re-injecting identity. `restart_count` increments in `native_status` (the only durable
   change); **no other durable state is touched**.
6. After the restart, the same `echo-sidecar.echo` call answers again — the sidecar resumed cleanly.
   The exit gate is met: a killed sidecar is restarted cleanly, and any durable workspace state
   (e.g. a channel message posted between steps 4 and 5) is **intact** across the restart (the child
   held none — the stateless-extension guarantee).
7. `native.stop` cooperatively shuts the child down (a `shutdown` notification, escalating to a
   process-group kill after a grace window); the runtime handle is dropped; `native_status` →
   `stopped`. `native.status` reports the final state.

## Testing plan

Mandatory categories (testing-scope §2) that apply here, plus the S7 supervision/restart category:

- **Capability-deny** (mandatory) — `denies_install_without_grant`: a principal lacking
  `mcp:native.install:call` is refused at the gate; **no child spawns, no record is written**.
  `denies_lifecycle_without_grant` for `stop`/`restart` likewise.
- **Workspace-isolation** (mandatory, store + MCP) — `ws_b_cannot_see_or_control_ws_a_sidecar`:
  ws-B's `native.status`/`stop`/`restart` for a ws-A sidecar resolve to nothing and are gate-denied
  (workspace-first); ws-B reading the store sees no ws-A `install`/`native_status` record.
- **Supervision / restart** (the S7 exit-gate category, testing §2 "hot-reload/offline where
  relevant" → here, restart) — `killed_sidecar_restarts_cleanly_with_no_durable_state_lost`: spawn
  a **real** child process, prove it answers a tool call, **kill the OS process**, assert the
  supervisor respawns it (restart_count increments), the respawned child answers the same tool call,
  AND a channel message posted before the kill is intact afterward. Plus
  `restart_policy_backs_off` (bounded restarts within a window — a crash-looping child is not
  respawned unboundedly) and `cooperative_stop_then_kill` (a `stop` drains via `shutdown`, escalating
  to kill after the grace window).
- **Install composition** — `native_artifact_installs_through_registry`: a `tier="native"` signed
  artifact pulls→verifies→caches→installs and spawns, proving the registry+native compose (the
  signature gate still runs; a tampered native artifact is rejected before spawn).
- **Manifest** — `ext-loader` unit: parse a `[native]` block (exec/args/health/restart), reject a
  native manifest missing `exec`, the `world` major check still applies.
- **Supervisor unit** (`lb-supervisor`): `Content-Length` framing round-trips (incl. a split read);
  a malformed frame is rejected; backoff schedule is correct; the `Launcher` fake child answers
  init/health/echo/shutdown.
- **Frontend** (Vitest): `NativeView` — install, see status (running + restart count), stop, and
  the deny-without-grant path, through the real api → invoke → fake.

Externals mocked: **a real OS process** for the supervision-restart proof (a true external,
testing §3) via a tiny reference child binary; the `Launcher` fake for the unit deny/isolation
paths. Real embedded SurrealDB + in-proc Zenoh everywhere else. Each test that boots a `Node` uses
`#[tokio::test(flavor = "multi_thread", worker_threads = 1)]` and a **unique workspace id**
(test-runner gotchas).

## Risks & hard problems

- **A child process is the first thing in this codebase that can leak across a crash.** A respawn
  that doesn't reap the old process group, or a health timer that fires during shutdown, leaves
  zombies or double-spawns. Mitigation: process-group spawn + kill-the-group on stop/restart;
  a single owned read-loop per child; the `restart_count` + bounded-window policy so a crash loop
  is capped, not infinite.
- **Determinism in tests.** OS timing (spawn latency, health intervals) is non-deterministic. The
  `Launcher`/`Backoff` are injectable; the one real-process test asserts *eventual* restart with a
  bounded poll (not a fixed sleep), and logical `ts` is injected as everywhere else.
- **The stateless invariant is easy to violate for a process.** It is tempting to keep "is it
  running, what's its PID, how many restarts" only in memory and treat that as authority. The rule:
  the **record** is authority (lifecycle intent + restart_count), the **runtime map** is a cache the
  record can rebuild. A boot reconciler (re-spawn `lifecycle=started` sidecars from records) is the
  natural consequence — scoped as a follow-up, not built this slice (single-process lifetime today).
- **Security boundary.** This is OS-process surface. The slice deliberately stops at process-group
  isolation + scoped identity + bounded restart and **flags** (here and in the session doc) that
  cgroup/seccomp/userns hardening is deferred — so no one reads "native tier shipped" as "native
  tier is fully sandboxed."

## Open questions

- ~~**`restart` as a first-class verb vs. only the crash policy.**~~ **RESOLVED:** ships both —
  the operator `native.restart` (cooperative stop→start) and the automatic `RestartPolicy` crash path
  (`call_sidecar` restarts-on-fault). Different triggers, distinct as intended.
- ~~**Where the native manifest fields live.**~~ **RESOLVED:** the `[native]` block (exec/args/target/
  restart), required for and exclusive to `tier="native"`, validated at parse (ext-loader).
- **Boot reconciler.** Re-spawn `lifecycle=started` sidecars from durable records on node boot (the
  rubix reconciler analogue). Deferred — single-process lifetime this slice; the `native_status`
  record shape supports it (additive). *Recorded in the session: scoped out, records ready.*
- **OS-level hardening depth** (cgroups/seccomp/userns) — deferred (non-goal of the minimal-sidecar
  posture). Which to add first when it lands?
- **Background health-poll reactor.** This slice restarts **on-demand at the call boundary**; a
  periodic `health` timer (+ the supervision events on the bus for observability) is the natural next
  step — a sidecar that crashes between calls is only noticed on the next call today.
- **The child→host callback transport.** This slice's sidecar uses only the control line; a sidecar
  that calls host MCP tools needs the routed-MCP callback wired (the deferred gateway/Tauri work).
  *Default: proxy through the control line (one transport), revisit if a firehose needs its own.*
- **Native artifact platform-target enforcement.** The `[native] target` field exists (carried into
  the catalog); refusing a binary built for the wrong target on install is the follow-up.

## Related

- `README.md` §6.3 (two tiers — this lands Tier 2), §6.4 (trust/install), §6.8 (authority).
- `extensions/extensions-scope.md` (the manifest; this adds the `[native]` block it deferred).
- `registry/registry-scope.md` + `sessions/registry/registry-session.md` (the slice this composes
  with — a native artifact installs through the same signed flow).
- `auth-caps/auth-caps-scope.md` (the grammar this deliberately does **not** change).
- `node-roles/node-roles-scope.md`, `platform-targets/platform-targets-scope.md` (filled alongside).
- `agent/agent-scope.md` (the delegation mint reused to scope the child's injected token).
- `STAGES.md` "Reuse: the extension server" + rubix-cube `extension-server-scope.md` (`supervisor/*`
  — the re-author source).
