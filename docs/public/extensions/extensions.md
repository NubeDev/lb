# Extensions — the runtime, the manifest, and the two tiers (as built)

The shipped extension model: what an extension *is*, the manifest contract it ships, and the **two
runtime tiers** it can run on. The full spec is README §6.3/§6.4; the asks are
`../../scope/extensions/`; the build logs are `../../sessions/`.

## What an extension is

An extension is a **stateless** unit of functionality the host loads, grants down to an approved
capability set, and exposes through the one MCP contract. It holds no durable state in its running
instance — its state lives in the store or on the bus, which is what makes hot-reload (S2) and
restart (S7) safe (§3.4). It is reached as `<id>.<tool>` MCP calls, gated `mcp:<id>.<tool>:call`.

**One extension = one folder; a backend and a frontend are each optional parts of it.** An extension
lives in a single directory (e.g. `rust/extensions/<id>/`) that may contain a **backend** (a wasm or
native runtime half) and/or a **frontend** (its own UI under `<id>/ui/`), in any combination: just a
backend (`echo-sidecar`), just a UI, wasm+UI, or native+UI (`fleet-monitor`). There is **no** separate
top-level `ui/extensions/` tree — a frontend is a part of its extension, beside its backend, not a
thing that lives apart from it. The reference `fleet-monitor` is the canonical native-backend +
federated-frontend example.

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

## A worked Tier-1 example: the `github-bridge`

The S6 coding workflow's inbound edge ships as an installed Tier-1 wasm extension (resolving the S6
deferral) — the second real extension after `hello`. It is a **pure transform**: its `normalize` tool
maps a raw GitHub webhook to the canonical `{ issue_id, payload, ts }` triple, holding no state and
making no host callback (it is a `@0.1.0` guest — it imports only `log`; the host-callback import
described below landed later and is opt-in, so a pure-transform guest never uses it).
The **host** composes it: `lb_host::ingest_via_bridge` calls the sandboxed `github-bridge.normalize`
tool, then hands the result to the host's `workflow.ingest_issue` write. Two independent capability
gates apply in order — `mcp:github-bridge.normalize:call` (the transform) then
`mcp:workflow.ingest_issue:call` (the must-deliver write) — and neither is widened. The split is the
point: the untrusted-input transform is sandboxed and swappable (a GitLab/Gitea bridge sharing the same
output contract drops in), while the state-mutating inbox write stays a host seam. The orchestrator
(triage → approval → job → outbox) remains a host service — it drives host-internal seams a guest can
only reach *through* MCP, never *be*. See `../../scope/extensions/github-bridge-scope.md`.

## The host-callback ABI: a guest calls host MCP tools (`@0.2.0`)

A guest used to be a **one-way box** — the host called *into* it (`tool.call`), but it could only
`host.log` back. So a backend that reads/writes the platform (a producer, a reactor, a "read a series →
derive another" tool) couldn't be a guest; it had to be a host service. The `@0.2.0` WIT minor bump adds
the **one** missing primitive — a host-mediated call-back — making a wasm extension a first-class
platform citizen:

```wit
// world extension's `host` interface, @0.2.0
call-tool: func(name: string, input-json: string) -> result<string, tool-error>;
```

This is the **symmetric backend dual of the page bridge**. The page reaches the MCP surface via `POST
/mcp/call` → `lb_host::call_tool`; a guest now reaches the *same* `call_tool`, the *same* verbs
(`series.*`, `ingest.write`, `outbox.status`, `inbox.list`/`resolve`, other extensions' `<ext>.<tool>`),
denied identically. One chokepoint, two front doors — **zero new trust surface**.

**Delegated, intersected authority.** Before running the guest the host derives its **effective
principal** = `caller ∩ install-grant`: the caller's caps INTERSECTED with the install's
`admin_approved` set (the S5 `Principal::derive` / caps gate 2b, the same `agent ∩ caller` the agent
loop uses). Every callback runs the full authorize gate (workspace-first, then `mcp:<tool>:call`)
against it. So a guest can reach **at most** what BOTH its caller and its install allow — never wider:

- a verb the **install grant omits** → denied, even when the caller holds it (delegation narrowing);
- a verb the **caller lacks** → denied, even when the install requested it (intersection both ways).

**Identity is per-call, never instance-sticky.** The host sets the effective principal + workspace + a
`HostBridge` handle into the instance's `HostState` *before* the guest runs and *clears it after*. The
loaded instance is node-global (one instance serves many workspaces — see
`../../debugging/extensions/loaded-extension-instance-is-node-global.md`), so a sticky identity would
leak across the wall; per-call is the only safe choice. The workspace is host-set from the caller's
token — never guest-supplied.

**Layering.** `lb-runtime` defines a narrow `HostBridge` trait + `CallContext`; `lb-host` implements it
over `call_tool`. So `runtime` stays *below* `host` in the dep graph — the forever-ABI addition doesn't
leak the host's shape into the SDK layer.

**Re-entrancy is bounded.** A guest→host→guest chain carries a depth counter; past a fixed
`MAX_CALL_DEPTH` (8) the callback returns `tool-error::failed("call depth exceeded")`. The callback
dispatches through `call_tool` (a *fresh* instance/route, `try_lock`ed) — it never re-borrows the
in-flight instance, so a self-re-entrant guest fails fast ("extension busy") instead of deadlocking.

**ABI compat.** World MAJOR stays `0`, so the loader still accepts `@0.1.0` guests — but a minor bump
turned out to break them at *instantiation* (wasmtime treats a `0.x` minor as semver-incompatible at
link time). The runtime links BOTH `host` versions and falls back to the frozen `@0.1.0` export bindings
(`sdk/wit-compat-0_1/`), so `hello`/`github-bridge` (`@0.1.0`) and a `@0.2.0` callback guest coexist on
one node. See `../../debugging/extensions/wit-minor-bump-breaks-0_1-guest-linking.md`.

**Reference.** `proof-panel`'s `proof.derive` reads the latest `proof.demo` (`series.latest`) and writes
`proof.derived = value*2` (`ingest.write`), entirely through the callback — a guest doing real platform
work, proven live (page → guest → host → store → page). See
`../../scope/extensions/host-callback-scope.md` and `../../sessions/extensions/host-callback-session.md`.

### The live ingress: the `github-webhook` role crate

`ingest_via_bridge` is a host helper; **`lb-role-github-webhook`** is the real HTTP edge that drives it
from an actual GitHub delivery (beside `lb-role-registry-host`; roles depend on host, never the
reverse). It is a node that also exposes one route, `POST /webhook`, and adds no authority. Two layers
guard it, in order:

1. **Transport authenticity** — `HMAC-SHA256(secret, raw-body)` against `X-Hub-Signature-256`, compared
   in **constant time** over the **raw bytes** GitHub signed (verifying re-serialized JSON would never
   match). A failure is an opaque `401` — no oracle, and the secret (mediated, crate-private) is never
   logged. The legacy SHA-1 `X-Hub-Signature` is deliberately not accepted.
2. **Capability + workspace** — a verified delivery calls `ingest_via_bridge` under a fixed
   principal/workspace, so the SAME two gates above and the workspace wall apply. An authentic delivery
   that lacks the grants is `403` (is GitHub, but unauthorized) — distinct from the `401` forgery case.

Idempotency on the normalized issue id makes GitHub's re-delivery one inbox item. `axum`/`hmac` live in
the role crate, never core. See `../../scope/extensions/github-webhook-scope.md`.

**The multi-tenant front door (S7).** One process fronts many workspaces: `tenant_router` exposes
`POST /webhook/{tenant}` over a `TenantRegistry` (an opaque slug → `{ws, principal, secret}` map), so
each repo points its Payload URL at its own `/webhook/{tenant}` with its **own** secret. Routing is by
the URL slug — chosen *before* the HMAC check, never by reading the unverified body — so
authenticity-before-parse holds and the **workspace wall holds at the front door**: a delivery signed
with one tenant's secret but sent to another's slug fails that tenant's HMAC (`401`) and never crosses
into its workspace. An **unknown tenant is the same opaque `401`** (not a `404`) — no enumeration
oracle. The single-tenant `/webhook` route stays for one-repo deployments; the front door is layered
beside it (no `if cloud`, one tenant per map row). `lb-secrets`-backed secrets + a dynamic tenant
directory (onboard without a restart) are the open follow-ups.

## The host-callback ABI: a guest calls host tools (WIT `@0.2.0`)

A Tier-1 guest is no longer a one-way box. The WIT world's `host` interface gained **one** import —
`host.call-tool(name, input-json) -> result<string, tool-error>` — so a guest can invoke the **same**
MCP tool surface the page bridge reaches (`series.*`, `ingest.write`, `outbox.status`,
`inbox.list`/`resolve`, and other extensions' `<ext>.<tool>`), getting JSON back, exactly as the
browser's `bridge.call(tool, args)` does. This is the §11.2 forever-ABI change — done once, behind the
existing `lb_host::call_tool` chokepoint, so it adds **zero** new trust surface beyond the browser bridge.

How the security holds, on every callback:

- **Identity is per-call, never instance-sticky.** Before running a guest, the host sets
  `{ effective_principal, ws, bridge }` into the instance's `HostState`; it is CLEARED after the call
  returns. The loaded instance is node-global (one instance serves many workspaces), so a sticky
  identity would leak across the wall — hence per-call. The runtime holds a narrow `HostBridge` trait
  object the host supplies (not an `Arc<Node>`), so `lb-runtime` stays below `lb-host` in the dep graph.
- **Effective principal = `caller ∩ install-grant`.** The host derives a principal whose caps are the
  intersection of the caller's caps and the extension's persisted install grant (`Principal::derive`,
  the S5 `agent ∩ caller` delegation primitive). A guest can reach **at most** what BOTH its caller and
  its install allow — a tool the install grant omitted is denied even if the caller holds it, and one
  the caller lacks is denied even if the install requested it. No widening, ever, at any re-entrancy
  depth.
- **The callback dispatches through the chokepoint.** `host.call-tool` →
  `lb_host::call_tool(node, &effective_principal, ws, name, input)` → the same authorize-then-dispatch
  any bridged caller runs, denied identically (opaque). The workspace is the host-set caller's, never
  guest-supplied. A re-entrant guest→host→guest chain is bounded by a fixed **depth guard** (8 →
  `tool-error::failed("call depth exceeded")`), and the callback resolves a FRESH instance/route
  (`try_lock` — a self-re-entry fails fast as "extension busy" rather than deadlocking on its own
  in-flight instance), never re-borrowing the in-flight `&mut Instance`.

**Versioning.** The WIT package bumped `@0.1.0` → `@0.2.0` (a MINOR add); the world MAJOR stays `0`. A
guest that *uses* the callback declares `@0.2.0`; existing `@0.1.0` guests (`hello`, `github-bridge`)
keep loading. (One subtlety: wasmtime's component linker treats a `0.x` minor as semver-incompatible at
*link* time, so the runtime links BOTH `host` versions and falls back to the frozen 0.1.0 export
bindings — see `../../debugging/extensions/wit-minor-bump-breaks-0_1-guest-linking.md`.)

**Worked example — `proof-panel.proof.derive`.** The reference guest's backend tool reads the latest
`proof.demo` (`series.latest`) and writes `proof.derived = value*2` (`ingest.write`), returning
`{ derived }` — entirely through the callback, under `caller ∩ grant`. It is the visible proof that a
wasm guest does real platform work, not just echo input. A guest never holds a `Store`/bus handle (rules
4/5); all platform access is through these host-mediated, gated MCP calls. The `[ui]` page's "Run derive"
card invokes it over the bridge and reads the committed `proof.derived` back. See
`../../scope/extensions/host-callback-scope.md` and `../../sessions/extensions/host-callback-session.md`.

**Producing workflow motion — the two write verbs + `proof.simulate`.** The callback initially exposed
only the READ/RESOLVE half of the durable-workflow surface (`outbox.status`, `inbox.list`,
`inbox.resolve`), so a guest could read motion but not PRODUCE it. Two write verbs now complete the
chokepoint (proof-workflow-sim scope), each gated identically (workspace-first, then `mcp:<verb>:call`,
against `caller ∩ install-grant`), reusing the real durable write paths — never a guest store handle:

- **`inbox.record`** `{channel, id, body, ts} -> {ok}` — create a durable inbox item (`lb_inbox::record`).
  The **author is host-forced** to the effective principal's `sub` (= `ext:<id>` for a guest callback —
  the extension acting on the caller's behalf), never caller-spoofable, like `inbox.resolve`'s actor.
- **`outbox.enqueue`** `{id, target, action, payload, ts} -> {ok}` — stage a must-deliver effect
  (`lb_outbox::enqueue`'s transactional change+effect, so the effect is never orphaned). Staged
  **Pending** — delivery stays the relay's job.

The reference guest's **`proof.simulate`** drives a full round-trip through these: `inbox.record` →
`inbox.list` (read it back) → `inbox.resolve` Approved → `outbox.enqueue` → `outbox.status`, returning
`{ inbox_id, resolved, outbox_pending }`. The `[ui]` page's "Run workflow simulation" card invokes it,
then refreshes the Inbox/Outbox sections so the produced item + effect become VISIBLE — the "I can finally
see it work" payoff. Each step authorizes per direction: a grant (or caller) missing `inbox.record` /
`outbox.enqueue` is denied at the host even when the other side holds it. See
`../../scope/extensions/proof-workflow-sim-scope.md` and
`../../sessions/extensions/proof-workflow-sim-session.md`.

## The frontend half: a federated UI page + dashboard widgets (S10)

An extension's manifest may declare a **`[ui]`** block (a full sidebar page) and zero or more
**`[[widget]]`** tables (dashboard palette tiles) — each frozen-field, serde-defaulted, independent of
the runtime tier (a wasm OR native extension may ship them). The host **projects** these onto the
durable `Install` (`crates/host/src/ui_decl.rs`, used by **both** the wasm and native install paths),
narrowing each declared `scope` to the install grant — a page/widget can never claim a tool the admin
didn't approve. `ext.list` surfaces them (`ExtRow.ui` / `ExtRow.widgets`), so the shell builds a
cap-gated nav slot + palette entries without re-reading the manifest.

**Trusted tier = real Module Federation (shared React).** A first-party page ships a Vite Module
Federation **remote** (`@originjs/vite-plugin-federation`) exposing `mount(el, ctx, bridge)` and
declaring `react`/`react-dom` as shared singletons. The shell is the federation **host**: it shares its
React, loads the remote's `remoteEntry.js` (served by the gateway at `GET /extensions/{ext}/ui/{*path}`,
traversal-guarded) via the federation runtime (`ui/src/features/ext-host/federation.ts`), and mounts it
**in-process** against the *same* React — so the page is native-feeling, not a bundled second copy. The
untrusted iframe-sandbox tier (same `mount` contract, postMessage transport) is the planned follow-up.

**The bridge is the only data path.** A page/widget reaches platform data ONLY through the
host-mediated `bridge.call(tool, args)` → `POST /mcp/call` → `lb_host::call_tool`, where the capability
and workspace are **re-checked per call**. The page never holds the session token, a DB handle, or
`invoke`; its reachable set is its install grant (for widgets, the frozen read-only series verbs). The
shell pre-filters out-of-scope tools (defense in depth); the host is the boundary.

**Reference extension: `fleet-monitor`** — a native Tier-2 sidecar (its own PID, `fleet.summary` MCP
tool) **plus** a co-located federated frontend (`rust/extensions/fleet-monitor/ui/`, real shadcn/ui +
Tailwind) that mounts a sidebar page with **3 nested routes** and declares **2 widget** tiles, all
reaching data through the bridge. See `../../scope/extensions/ui-federation-scope.md`,
`../../scope/frontend/dashboard-widgets-scope.md`, and
`../../sessions/extensions/fleet-monitor-federation-session.md`.

**Reference extension: `proof-panel`** — the **Tier-1 WASM** counterpart, the first shipped artifact
that proves the whole basics composed end-to-end on the in-process path: one self-contained folder
(`rust/extensions/proof-panel/`) carrying **both** a real MCP tool served from the wasm guest
(`proof.ping`, stateless, returns a workspace-tagged `{"ok","ws","node":"proof-panel","tier":"wasm"}`
snapshot) **and** a co-located federated page that proves the platform **end to end from one cap-gated
page** through the bridge — the "whole platform, one page" demo. The page now exercises the full
round-trip, not just the read half:

- **Ingest → read round-trip (the page creates its own data):** a "Write sample" button →
  `ingest.write { samples }` → `series.latest` reads it back live (write → stage → drain → read, in the
  browser).
- **Outbox status:** a card of `outbox.status` `{pending,delivered,dead_lettered}` + Refresh.
- **Inbox triage:** `inbox.list { channel }` items with Approve/Reject → `inbox.resolve { item_id,
  decision }` (the page's first durable-workflow WRITE; the actor is host-forced to the principal).
- **Browse series:** the original `series.find`/`series.latest` read half.

Installing it exercises publish → grant-intersection → mount → cap-checked call → workspace-scoped
result in one motion, with **no placeholders** (the gap `fleet-monitor`'s widgets left). It ships **no
`[[widget]]`** (deferred to the dashboard scope). Three guarantees it pins down that nothing else did:

- **Grant intersection is enforced at call time, not just displayed.** Install with an approval that
  omits `series.latest` → the persisted page scope drops it AND a bridge `series.latest` call by the
  page's principal is denied server-side (an honest error, never a blank).
- **The bridge actually reaches host-native verbs.** `proof-panel` surfaced (and fixed) that
  `POST /mcp/call` → `lb_host::call_tool` resolved only the runtime **registry**, so host-native
  `series.*`/`ingest.*` verbs `NotFound`-ed — a federated page could never read a series through the
  bridge. `call_tool` now authorizes (same MCP gate) then dispatches host-native verbs:
  `series.*`/`ingest.*` via `call_ingest_tool`, and the durable-workflow surface
  (`outbox.status`/`inbox.list`/`inbox.resolve`) via a workflow dispatcher; extension `<ext>.<tool>`
  calls still route through the registry unchanged. No new verb, no WIT change. See
  `debugging/extensions/bridge-cannot-dispatch-host-native-series.md`.
- **A federated page can WRITE, not just read — through the same gate.** `ingest.write` over the bridge
  drains staging synchronously (mirroring `POST /ingest`; there is no background drain worker), so the
  page reads back what it just wrote in one motion. `inbox.resolve` mutates durable workflow state with
  the actor host-forced. Every write verb has its own deny-test and the workflow surface is
  workspace-isolated (ws-B sees none of ws-A's items/effects).

The wasm tool is proven through the real `lb-runtime` component (`crates/host/tests/proof_panel_test.rs`,
9 tests incl. the ingest/outbox/inbox round-trips + per-verb deny + isolation); the page's data path is
proven against a **real spawned gateway** (`ui/src/features/ext-host/ProofPanel.gateway.test.tsx`, 9
tests, seeding real series/inbox/outbox via the test gateway's `/_seed/*` routes) and end-to-end in a
real browser (`ui/e2e/proof-panel.spec.ts`: click Write sample → the committed value renders; Refresh
outbox → counts render; no hook/console errors). See
`../../scope/extensions/proof-panel-scope.md` and `../../sessions/extensions/proof-panel-session.md`.

## Placement & targets

`placement` (`local-only`/`cloud-only`/`either`) is matched against a node's **role** as data, never
an `if cloud` branch (`../../scope/node-roles/node-roles-scope.md`). A **native** binary is
platform-specific (unlike a portable `.wasm`), so a native artifact also carries a target triple a
node matches on install (`../../scope/platform-targets/platform-targets-scope.md`).

## Related

`registry/registry.md` (signed distribution), `mcp/mcp.md` (the call contract), `auth-caps/auth-caps.md`
(the grant model), `files/files.md` (install records), `../SCOPE.md` (the shipped index).
