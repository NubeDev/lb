# Extensions — the runtime, the manifest, and the two tiers (as built)

The shipped extension model: what an extension *is*, the manifest contract it ships, and the **two
runtime tiers** it can run on. The full spec is README §6.3/§6.4; the asks are
`../../scope/extensions/`; the build logs are `../../sessions/`.

## What an extension is

An extension is a **stateless** unit of functionality the host loads, grants down to an approved
capability set, and exposes through the one MCP contract. It holds no durable state in its running
instance — its state lives in the store or on the bus, which is what makes hot-reload (S2) and
restart (S7) safe (§3.4). It is reached as `<id>.<tool>` MCP calls, gated `mcp:<id>.<tool>:call`.

## The manifest (`extension.toml`)

The §13 forever contract — TOML, parsed before anything is instantiated (so a denied call is refused
without ever starting the extension). It declares identity, **tier**, placement, the capabilities it
**requests** (a request, never a grant — the host grants `requested ∩ admin_approved`), its tools,
and visibility. A `tier="native"` manifest additionally carries a `[native]` block (exec/args/target/
restart) — required for and exclusive to the native tier. See `../../scope/extensions/extensions-scope.md`
and `../../scope/extensions/native-tier-scope.md`.

## Two runtime tiers (one control plane)

`install` / lifecycle / `status` are the **same verbs** whether the backend is wasm or native — the
tier is an implementation detail behind one surface, not a forked subsystem.

### Tier 1 — WASM (the default, S1)

A WebAssembly Component loaded into an in-process wasmtime runtime, invoked over the versioned WIT
world. Portable (one `.wasm` runs on every node), capability-sandboxed (it gets nothing the host
doesn't grant via WIT imports). The default for sandboxable, portable logic. See `crate-layout/`,
`mcp/mcp.md`.

### Tier 2 — native (the escape hatch, S7)

A real OS child process the host **supervises** — for an extension that needs its own socket, thread,
or long-lived daemon (a language server, an MQTT bridge). The shipped mechanics:

- **`lb-supervisor`** owns the OS plumbing behind a `Launcher` seam: spawn → `Content-Length`
  JSON-RPC handshake (`init`) → health poll → cooperative `shutdown` (escalating to a process-group
  kill) → `restart` (kill + relaunch from the spec, bounded by exponential backoff + a restart
  budget so a crash loop is capped).
- **The host `native` service** drives it: `install_native` persists the durable `Install` record
  then spawns; `call_sidecar` dispatches a child tool and **restarts-on-fault then retries** (the
  supervision crash-path); `stop`/`restart`/`status` are the operator controls.
- **Stateless supervision.** The live `Sidecar` (PID, stdio, restart count) is **runtime-only** (a
  `SidecarMap` keyed `(ws, ext_id)`); the durable truth is the `Install` record + a `native_status`
  projection (lifecycle intent + restart count) in the workspace namespace. A restart re-derives from
  the records → **no durable state is lost** (§3.4 applied to a process).
- **Scoped identity.** The child is spawned with `LB_EXT_WS`/`LB_EXT_ID`/`LB_EXT_TOKEN` in its env —
  a token minted carrying exactly the granted set. A compromised child is bounded by its scoped key +
  process-group isolation.
- **Posture.** Process-group isolation + scoped identity + bounded restart. OS-level hardening
  (cgroups/seccomp/userns) and a boot reconciler are noted follow-ups (`../../scope/extensions/native-tier-scope.md`).

The reference Tier-2 extension is `echo-sidecar` (a real host-platform binary speaking the
supervisor's wire types verbatim — the child↔host ABI cannot drift, the native peer of the wasm tier
sharing the WIT world).

## Install & trust (S4 + S7)

Install persists `granted = requested ∩ admin_approved` as a durable `install:{ext_id}` record per
workspace (`installed` reads it back, workspace-isolated). A signed artifact from the **registry**
(S7) installs through the **same** flow with a verified pull in front — `install_from_registry` (wasm)
/ `install_native_from_registry` (native). **Two independent gates** apply throughout: the
**capability** gate (`mcp:*` , workspace-first) and the **signature** gate (`verify_artifact`).
Granted ≠ trusted; trusted ≠ granted. See `registry/registry.md`.

## Placement & targets

`placement` (`local-only`/`cloud-only`/`either`) is matched against a node's **role** as data, never
an `if cloud` branch (`../../scope/node-roles/node-roles-scope.md`). A **native** binary is
platform-specific (unlike a portable `.wasm`), so a native artifact also carries a target triple a
node matches on install (`../../scope/platform-targets/platform-targets-scope.md`).

## Related

`registry/registry.md` (signed distribution), `mcp/mcp.md` (the call contract), `auth-caps/auth-caps.md`
(the grant model), `files/files.md` (install records), `../SCOPE.md` (the shipped index).
