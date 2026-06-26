# Native tier — the supervised Tier-2 sidecar (session)

- Date: 2026-06-26
- Scope: ../../scope/extensions/native-tier-scope.md
- Stage: S7 — platform maturity (STAGES.md). The **second** S7 vertical slice; closes the remaining
  half of the S7 exit gate ("a native sidecar is supervised and restarts cleanly").
- Status: done

## Goal

Build the **native Tier-2 supervisor** end to end: a host service that spawns an OS child process
beside the wasm tier, performs a framed JSON-RPC handshake, health-checks it, restarts it cleanly
on crash (bounded backoff), and cooperatively stops it — proven with a reference sidecar extension,
with **no durable workspace state lost** across a restart (the stateless-extension guarantee carried
into Tier 2). This is the remaining half of the S7 exit gate.

## What changed

Scope authored first (the native manifest fields + node-roles + platform-targets were
stubs/deferred, per HOW-TO-CODE/SCOPE-WRITTING):
[native-tier-scope.md](../../scope/extensions/native-tier-scope.md), and the two stubs filled:
[node-roles-scope.md](../../scope/node-roles/node-roles-scope.md) (placement × role for a process),
[platform-targets-scope.md](../../scope/platform-targets/platform-targets-scope.md) (the target tag
a native binary needs).

New crate **`lb-supervisor`** — the OS plumbing + supervision policy, behind a `Launcher` seam (the
registry's `Source` analogue), one verb per file:
- [spec.rs](../../../rust/crates/supervisor/src/spec.rs) — `Spec`/`RestartPolicy`/`Backoff` (the
  recipe re-read verbatim on every respawn → a restart is identical to the first spawn).
- [rpc.rs](../../../rust/crates/supervisor/src/rpc.rs) — the closed child wire protocol
  (`init`/`health`/`call`/`shutdown`).
- [frame.rs](../../../rust/crates/supervisor/src/frame.rs) — `Content-Length` JSON-RPC framing over
  the child's stdio (split-read tolerant, 16 MiB cap).
- [launcher.rs](../../../rust/crates/supervisor/src/launcher.rs) — the `Launcher` trait + `Channel`/
  `Kill` (spawn behind a trait → testable with a fake child or a real process).
- [os.rs](../../../rust/crates/supervisor/src/os.rs) — the real OS launcher (`OsLauncher`), the ONE
  process-boundary file: spawns in its own process group so a kill reaps grandchildren.
- [sidecar.rs](../../../rust/crates/supervisor/src/sidecar.rs) — the live `Sidecar`: handshake ·
  correlated `call` · `health` · cooperative `shutdown` · `restart` (kill+relaunch, budget-bounded).

New reference extension **`echo-sidecar`** (a real host-platform binary, a workspace member — UNLIKE
the wasm `hello`): [main.rs](../../../rust/extensions/echo-sidecar/src/main.rs) reads its injected
scoped identity from env and serves the protocol using the SAME `lb-supervisor` wire types (the
child↔host ABI cannot drift — the native peer of the wasm tier sharing the WIT world). Its `crash`
tool replies-then-exits (a deterministic crash for the restart proof).

Manifest **`[native]` block** ([manifest.rs](../../../rust/crates/ext-loader/src/manifest.rs)): a
`Native` struct (exec/args/target/restart), required for and exclusive to `tier="native"` (validated
at parse — the deferred extensions-scope "Native manifest fields (exec, supervision, socket) — S7").

New host **`native` service** (beside `agent`/`channel`/`assets`/`workflow`/`registry`), one verb
per file:
- [registry.rs](../../../rust/crates/host/src/native/registry.rs) — the runtime `SidecarMap` (live
  children keyed `(ws, ext_id)`; never the store — the PID is motion).
- [status.rs](../../../rust/crates/host/src/native/status.rs) — the durable `native_status`
  projection (lifecycle intent + restart count, workspace-namespaced).
- [spec.rs](../../../rust/crates/host/src/native/spec.rs) — build a `lb_supervisor::Spec` from a
  manifest + mint & inject the child's scoped identity token (`requested ∩ admin_approved`).
- [authorize.rs](../../../rust/crates/host/src/native/authorize.rs) — the
  `mcp:native.<verb>:call` gate (workspace-first), like `authorize_registry`.
- [install.rs](../../../rust/crates/host/src/native/install.rs) — `install_native`: persist the S4
  `Install` record → spawn → record status (the start verb).
- [lifecycle.rs](../../../rust/crates/host/src/native/lifecycle.rs) — `stop`/`restart`/`status`.
- [tool.rs](../../../rust/crates/host/src/native/tool.rs) — the `native.*` MCP bridge (store-only
  `status`) + `call_sidecar` (child dispatch with crash-restart-on-fault — the supervision proof).

Registry × native composition:
[registry/install_native.rs](../../../rust/crates/host/src/registry/install_native.rs) —
`install_native_from_registry`: pull · **verify** · write the binary to disk · supervise. A signed
`tier="native"` artifact installs through the same flow a wasm one does; both gates hold.

Node wiring: `Node` gains `sidecars: Arc<SidecarMap>` ([boot.rs](../../../rust/crates/host/src/boot.rs));
host `lib.rs` re-exports the native verbs. Workspace: `lb-supervisor` + `echo-sidecar` added as
members; tokio gains `process`/`io-util`/`time` (additive — core crates use the S1 subset).

UI **`native` feature** (mirrors the RegistryView slice): `native.types`, `native.api` (one call per
verb), a faithful in-memory `native.fake` (capability gate + supervision + isolation), `useNative`,
`NativeView` (surfaces the restart count + running flag — the supervision, visible), and a Vitest
spec. Wired into the `fake.ts` dispatcher.

## Decisions & alternatives

- **Spawn is gated as a host-native MCP verb (`mcp:native.<verb>:call`), NOT a new `process:`
  capability surface.** The grammar has exactly four surfaces and "a new surface is a deliberate
  grammar change" (`caps/request.rs`). Spawning is authority already expressible as "may call
  `native.install`" — gating it through the proven `authorize_tool` chokepoint costs ZERO grammar
  change and makes the deny/isolation tests identical to the registry's. **Flagged loudly** (the
  non-negotiable): this slice touches the OS-process boundary but does **not** touch the SDK/WIT
  world or the capability grammar. Rejected the `process:` surface as a forever decision bought for
  no capability the MCP gate can't already express.
- **The supervisor is a seam (`Launcher`), like `Source`/`Target`/`ModelAccess`.** The host service
  drives it; tests inject a fake child for the deny/isolation/unit paths and a **real OS process**
  for the supervision-restart proof (mock only the true external — a real process IS the external,
  testing §3). Rejected a polymorphic `Engine` growing a native backend: a wasm in-process call and
  an async OS child with stdio framing + health + restart are genuinely different responsibilities
  (FILE-LAYOUT blast-radius) — they share the control plane + identity model, not the runtime.
- **Supervision state is runtime-only; the durable truth is a record.** The live `Sidecar` (PID,
  stdio, restart_count) lives in the runtime `SidecarMap`; the durable state is the S4 `Install`
  record (now for native too) + the `native_status` projection. A restart re-derives from the
  records → no durable state lost (§3.4 applied to a process). Rejected keeping running-state/PID as
  authority.
- **Crash-restart is applied on-demand at the call boundary (this slice), not a background health
  timer.** `call_sidecar` detects a dead child (transport fault), restarts via the launcher, and
  retries once — the supervision crash-path, deterministic in tests (no timer to race). A background
  health-poll reactor + a boot reconciler are the natural next step (scoped as follow-ups; the
  records already support them).
- **The child's identity is an injected scoped token** minted carrying exactly `granted`, in the
  child's env (`LB_EXT_WS`/`LB_EXT_ID`/`LB_EXT_TOKEN`). A compromised child is bounded by its scoped
  key + process-group isolation. Verifying the token host-side is the deferred token-on-the-bus work
  (the same co-trust posture as `Principal::routed`).
- **Sandbox posture = minimal proven sidecar** (the user's call): process-group isolation + scoped
  identity + bounded restart this slice; cgroups/seccomp/userns hardening is a noted follow-up,
  flagged so "native tier shipped" is not read as "native tier fully sandboxed".

## Tests

Mandatory categories (testing-scope §2) + the S7 supervision/restart category, all green. Externals
mocked: a **real OS process** for the supervision-restart proof (the reference `echo-sidecar`
binary) + the `Launcher` fake for the deny/isolation/unit paths. Real embedded SurrealDB + in-proc
Zenoh everywhere else.

- **Supervision / restart** (the S7 exit-gate category) — host
  [native_test.rs](../../../rust/crates/host/tests/native_test.rs):
  `killed_sidecar_restarts_cleanly_with_no_durable_state_lost` (REAL process: install → answers a
  tool call tagged with the injected identity → crash → next call restarts cleanly + answers,
  restart_count=1 → a channel message posted before the crash is INTACT after → cooperative stop) and
  `native_artifact_installs_through_registry` (a signed `tier="native"` artifact pulls→verifies→
  writes→supervises; a tampered native artifact is rejected by the signature gate even with the grant).
- **Capability-deny** (mandatory) —
  [native_deny_test.rs](../../../rust/crates/host/tests/native_deny_test.rs):
  `denies_install_without_grant` (the launcher is NEVER reached, no record written),
  `denies_stop_without_grant` (a granted sidecar survives a denied stop).
- **Workspace-isolation** (mandatory, store + MCP) —
  [native_isolation_test.rs](../../../rust/crates/host/tests/native_isolation_test.rs):
  `ws_b_cannot_see_or_control_ws_a_sidecar` (ws-B's store read → None; ws-B principal targeting ws-A
  → workspace-first deny; ws-B's own map has no such sidecar → NotRunning; ws-A's child untouched).
- **Supervisor unit** (`lb-supervisor`) —
  [sidecar_test.rs](../../../rust/crates/supervisor/tests/sidecar_test.rs) + inline: handshake/call/
  health, restart relaunches + increments, restart budget bounded, `Never` policy refuses, shutdown
  ends the sidecar; Content-Length framing round-trips (incl. split read + EOF + case-insensitive).
- **Manifest** (`lb-ext-loader` inline): parse `[native]`, reject a native manifest missing exec,
  reject a wasm manifest carrying `[native]`, wasm omits native.
- **Frontend** (Vitest,
  [NativeView.test.tsx](../../../ui/src/features/native/NativeView.test.tsx)): install (running, 0
  restarts), restart (count increments — the supervision, surfaced), stop (flips off), deny — through
  the real api → invoke → fake path.

Green output:

```
##### lb-supervisor #####
running 5 tests (unit: frame round-trip/split/eof/header, backoff)
test result: ok. 5 passed; 0 failed
running 5 tests (sidecar_test: handshake+call+health, restart+count, budget, never, shutdown)
test result: ok. 5 passed; 0 failed

##### lb-ext-loader (incl. 4 new native manifest) #####
test result: ok. 6 passed; 0 failed

##### host: native_test (REAL process) #####
test killed_sidecar_restarts_cleanly_with_no_durable_state_lost ... ok
test native_artifact_installs_through_registry ... ok
test result: ok. 2 passed; 0 failed

##### host: native_deny_test #####
test denies_install_without_grant ... ok
test denies_stop_without_grant ... ok
test result: ok. 2 passed; 0 failed

##### host: native_isolation_test #####
test ws_b_cannot_see_or_control_ws_a_sidecar ... ok
test result: ok. 1 passed; 0 failed
```

Regression — every other host `--test` binary passes (spine, install_record, hot_reload, registry ×4,
workflow ×3, agent ×4, assets ×4, messaging ×3, sync, presence, cross-node) + `lb-registry` (10):

```
spine_test: 4 ok    install_record_test: 2 ok    hot_reload_test: 2 ok
registry_test: 4 ok    registry_rollback_test: 2 ok    registry_offline_test: 3 ok    registry_isolation_test: 2 ok
workflow_test: 3 ok    agent_test: 4 ok    assets_doc_test: 6 ok    ...   (all green)
lb-registry: 10 ok
```

```
# UI
 ✓ src/features/native/NativeView.test.tsx (4 tests)
 Test Files  8 passed (8)   Tests  26 passed (26)
# tsc --noEmit: clean
```

`cargo fmt --check`: clean. `scripts/check-file-size.sh`: all 299 source files ≤400 lines.

Totals after this slice: **~163 Rust + 26 Vitest + 2 shell** green (was 145 + 22 + 2 at the registry
slice; +10 `lb-supervisor` + 4 `lb-ext-loader` native + 5 host native = ~18 Rust, +4 Vitest).

## Debugging

None — nothing broke that warranted a `debugging/` entry. The one design refinement worth noting (not
a bug): the first crash-test draft drove the crash via a global `ECHO_PANIC_ON` env var, which was
fragile (the running child wouldn't inherit a late-set var, and the `crash`-loops-the-budget problem).
Replaced with a `crash` tool that **replies then exits** — so "induce the crash" (the `crash` call
succeeds) is cleanly separate from "verify the restart" (the NEXT call respawns + answers). Recorded
here, not a product bug.

## Public / scope updates

- Promoted to [public/extensions/extensions.md](../../public/extensions/extensions.md) (the shipped
  runtime/tier truth) and added the S7 native-tier row to [public/SCOPE.md](../../public/SCOPE.md).
- Resolved the extensions-scope deferral ("Native manifest fields (exec, supervision, socket) — S7")
  → the `[native]` block ships. Filled the `node-roles` + `platform-targets` stubs. Refreshed the
  native-scope open questions (boot reconciler, OS hardening, child→host callback transport, native
  platform-target enforcement) as the native-tier follow-ups.

## Dead ends / surprises

- `tokio` in the workspace omitted `process`/`io-util`/`time`; added them (additive — core crates
  still use only the S1 subset). The `echo-sidecar` child also needs `io-std` for its own stdin/
  stdout, so it carries its own tokio (a leaf binary sets its own runtime features).
- The composition test copies the 17 MB sidecar binary through the SurrealDB cache several times, so
  it runs ~30s — acceptable (one test, a real artifact round-trip). Noted for anyone surprised by it.

## Follow-ups

Pushed to the native scope's open questions:
- **Boot reconciler** — re-spawn `lifecycle=started` sidecars from the durable records on node boot
  (the rubix reconciler analogue). The records support it; single-process lifetime this slice.
- **OS-level hardening** — cgroups/seccomp/userns (the deferred non-goal of the minimal-sidecar posture).
- **Background health-poll reactor** — this slice restarts on-demand at the call boundary; a periodic
  health timer (+ the supervision events on the bus) is the natural next step.
- **Child→host MCP callback transport** — wire the routed-MCP callback so a sidecar can call host
  tools with its injected token (the deferred gateway/Tauri work; mirrors the registry's HTTP `Source`).
- **Native artifact platform-target enforcement** — refuse a binary built for the wrong target on
  install (the `[native] target` field exists; matching is the follow-up).
- Gateway/Tauri wiring for `native_*` (mirrors the S3 channel transport swap), like the S4/S5/S6/S7 verbs.
- STATUS.md updated: the native-tier slice is **shipped**; the **S7 exit gate is now fully MET**.
