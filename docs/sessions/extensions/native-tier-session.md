# Native tier — the supervised Tier-2 sidecar (session)

- Date: 2026-06-26
- Scope: ../../scope/extensions/native-tier-scope.md
- Stage: S7 — platform maturity (STAGES.md). The **second** S7 vertical slice; closes the remaining
  half of the S7 exit gate ("a native sidecar is supervised and restarts cleanly").
- Status: in-progress

## Goal

Build the **native Tier-2 supervisor** end to end: a host service that spawns an OS child process
beside the wasm tier, performs a framed JSON-RPC handshake, health-checks it, restarts it cleanly
on crash (bounded backoff), and cooperatively stops it — proven with a reference sidecar extension,
with **no durable workspace state lost** across a restart (the stateless-extension guarantee carried
into Tier 2). This is the remaining half of the S7 exit gate.

## What changed

(filled as work proceeds — scope authored first since the native manifest fields + node-roles +
platform-targets were stubs/deferred per HOW-TO-CODE/SCOPE-WRITTING.)

- Authored [scope/extensions/native-tier-scope.md](../../scope/extensions/native-tier-scope.md)
  (the ask) before any code.
- Filled the two stubs the slice touches:
  [node-roles-scope.md](../../scope/node-roles/node-roles-scope.md) (placement × role for a process)
  and [platform-targets-scope.md](../../scope/platform-targets/platform-targets-scope.md) (the
  target tag a native binary needs).

## Decisions & alternatives

- **Spawn is gated as a host-native MCP verb (`mcp:native.<verb>:call`), NOT a new `process:`
  capability surface.** A new surface is a deliberate grammar change (`caps/request.rs`); the MCP
  gate already expresses "may spawn" as "may call `native.install`", reusing the proven
  registry/workflow host-service gate with zero grammar change. Flagged loudly: this slice touches
  the OS-process boundary but does **not** touch the SDK/WIT world or the capability grammar.
- **The supervisor is a seam (`Launcher`), like `Source`/`Target`/`ModelAccess`.** A new
  `lb-supervisor` crate owns the OS plumbing; the host `native` service drives it. The `Launcher`
  trait lets tests inject a fake child for deny/isolation unit paths and a real OS process for the
  supervision-restart proof (mock only the true external — a real process is the external).
- **Supervision state is runtime-only; the durable truth is a record.** The live `Sidecar` (PID,
  stdio, restart_count) lives in a runtime map on the `Node`; the durable state is the S4 `Install`
  record (now for native too) + a `native_status` projection. A restart re-derives from the record →
  no durable state lost (§3.4 applied to a process). Rejected keeping PID/running-state as authority.
- **`lb-supervisor` is its own crate, not a polymorphic `Engine`.** Wasm-in-process and a native
  child are genuinely different responsibilities (FILE-LAYOUT blast-radius); they share the control
  plane + identity model, not the runtime. The host `native` service is the one dispatch point.

## Tests

(pasted green output goes here when the suite runs — mandatory: capability-deny,
workspace-isolation, supervision/restart with no durable state lost; plus install-composition,
manifest parse, supervisor unit, frontend Vitest.)

## Debugging

None yet.

## Public / scope updates

(filled on ship — promote to public/extensions/extensions.md + public/SCOPE.md; resolve the native
scope's open questions; fill node-roles/platform-targets stubs — done.)

## Dead ends / surprises

(filled as encountered.)

## Follow-ups

- Boot reconciler (re-spawn `lifecycle=started` from records); OS-level hardening
  (cgroups/seccomp/userns); child→host MCP callback transport; native artifact platform-target
  enforcement. All recorded in the native scope's open questions.
- STATUS.md to move on ship (S7 exit gate second half).
