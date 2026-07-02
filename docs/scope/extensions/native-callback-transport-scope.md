# extensions scope — the native-sidecar → host callback transport (`lb-sidecar-client`)

Status: scope (the ask). Promotes to `public/extensions/` once shipped. Prerequisite for
control-engine **S4** (`scope/control-engine/slice-4-appliance-registry-routing.md`) and
**S5** (write verbs → `outbox.enqueue`).

A native (Tier-2) sidecar — a supervised OS child the host spawns with `lb-supervisor` — today
is a **one-way box**: the host calls *into* the child over `Content-Length`-framed stdio
(`Init`/`Health`/`Call`/`Shutdown`), and the child can answer, but it **cannot call back into the
platform** — no `ingest.write`, no `outbox.enqueue`, no `store.*`, no other extension's
`<ext>.<tool>`. This is the out-of-process twin of the gap the **WASM** host-callback already
closed (`host-callback-scope.md`: the in-process `host.call-tool` WIT import). We want the
symmetric backend half for native tier: a small `lb-sidecar-client` a sidecar uses to call the
**same** MCP tool surface the browser bridge and the WASM guest already reach — an authenticated
`POST /mcp/call` to the gateway — under the child's **scoped, intersected authority**,
capability- and workspace-checked on every call. This adds **zero** new trust surface beyond
what the page bridge already has: it is one more transport for the one MCP contract (rule 7).

## Goals

- A native sidecar can call **any host-native or extension MCP tool it is granted** —
  `store.write`/`store.delete` (see `scope/store/store-write-verb-scope.md`), `ingest.write`,
  `outbox.enqueue`, `series.*`, and `<ext>.<tool>` — from inside its `Call` handler, getting
  JSON back, exactly as the page's `bridge.call(tool, args)` and the WASM guest's
  `host.call-tool` do.
- The call is **authorized at the host**: workspace-first, then `mcp:<tool>:call`, against the
  child's scoped token — `granted = requested ∩ admin_approved` (the same intersection the WASM
  and native install already computes). A sidecar can never reach a tool its install grant
  omitted.
- **The child authenticates with a verifiable, node-signed token.** The supervisor mints the
  child's `LB_EXT_TOKEN` with the **node's** signing key (not a throwaway), so the gateway's
  existing `session::authenticate` verifies it on the callback — closing the co-trust gap the
  native-tier scope deferred (the minter here and the verifier there share `Node::key`).
- **One small reusable crate, `lb-sidecar-client`**, that every native sidecar shares
  (`federation`, `fleet-monitor`, the ROS driver, `control-engine`) — not re-invented per
  extension. One method: `call_tool(tool, input) -> Result<Value, CallError>`.
- The reference native sidecar gets a **real callback that uses it**, so the demo proves a child
  doing real platform work, not just echoing stdio.

## Non-goals

- **No new tool surface.** The transport dispatches the *existing* verbs through the *existing*
  gateway `POST /mcp/call` → `lb_host::call_tool` chokepoint. It adds no CRUD/list/watch verbs —
  those are each their own scope (the generic `store.write` it will call is
  `scope/store/store-write-verb-scope.md`, scoped separately).
- **No raw store/bus handle in the child.** A sidecar never gets a `Store` or Zenoh handle
  (rule 5: host-mediated; rule 4: stateless). DB access is *only* via host MCP verbs through the
  callback.
- **No streaming/`watch` from a sidecar** this slice (request/response only). Guest-initiated
  motion (`ce.watch`'s arm side, S6) is a separate concern built on the extension-watch
  primitive.
- **No change to the WASM path.** `host-callback-scope.md` already shipped the in-process half;
  this is the out-of-process dual, sharing the same host chokepoint and the same deny semantics.
- **No new gateway route.** Reuses the existing `POST /mcp/call` the page bridge already posts to.

## Intent / approach

**One chokepoint, three front doors.** The host already funnels every bridged tool call through
`lb_host::call_tool(node, principal, ws, tool, input)`; the page reaches it via `POST /mcp/call`,
the WASM guest via the `host.call-tool` import. The native sidecar gets the **same** gateway
route over authenticated HTTP. Browser, WASM guest, and native child become three transports for
the one MCP contract, each denied identically.

Three pieces (mirrors the ui-ext implementation, which this ports cleanly):

1. **A node-signed child token.** `native/spec.rs` already injects `LB_EXT_WS`/`LB_EXT_ID`/
   `LB_EXT_TOKEN`; today the token is signed with a throwaway key (co-trust, unverifiable). We
   change `build_spec` to mint it with the **node's** `SigningKey` (threaded in from `boot.rs` →
   `install_native`), and to inject **`LB_GATEWAY_URL`** when a gateway fronts the node. The
   token carries exactly `granted = requested ∩ admin_approved`, `sub = ext:{id}`, the child's
   `ws`, `Role::Member`, `iat=0`/`exp=MAX` (bounded by process lifetime + its cap set, like the
   other in-process tokens).

2. **The `lb-sidecar-client` crate.** Two responsibilities, two files (FILE-LAYOUT):
   `config.rs` reads `LB_EXT_WS`/`LB_EXT_ID`/`LB_EXT_TOKEN`/`LB_GATEWAY_URL` from env once
   (empty = absent, distinct `NoToken`/`NoGateway` errors); `client.rs` holds one pooled
   `reqwest::Client` and exposes `call_tool(tool, input)` → `POST {gateway}/mcp/call` with
   `{tool, args}` body + `Authorization: Bearer {token}` (ws is **never** on the wire — the host
   derives it from the token, §7). A `403` maps to `CallError::Denied` (the one status a sidecar
   must distinguish — opaque, no reason oracle); other statuses to `Http`; connect/timeout to
   `Transport`.

3. **The gateway verifies it.** The existing `POST /mcp/call` `session::authenticate` already
   verifies bearer tokens against the node key; because the minter (spec.rs) and verifier
   (gateway) now share `Node::key`, the child's token verifies and `call_tool` runs the full
   authorize-then-dispatch. No route change; only the shared-key wiring (`gateway/src/state.rs`
   carries the node key).

**Why this and not alternatives.** (a) *Give the sidecar a direct `Store`/bus handle* —
rejected: breaks rules 4/5, a second un-gated data path, and the whole point is a child bounded
by its grant. (b) *A bespoke stdio callback method (child→host over the existing pipe)* —
rejected: it would duplicate the entire authorize-then-dispatch + identity plumbing on a second
transport, and the child already has a network identity (its token); reusing `POST /mcp/call`
means the native child is authorized by the **exact same code** as the page and the WASM guest,
with no divergence. (c) *Keep sidecars one-way and force their backend logic into host services*
— the status quo; rejected because it makes "ship a native backend in your extension" a lie
(control-engine's registry CRUD, ROS's setpoint outbox, all need it). The generic HTTP callback
is the minimal, principled, once-and-done answer, and it is already proven on `ui-ext`.

## How it fits the core

- **Tenancy / isolation:** the callback's workspace is the one **inside the child's token**
  (host-set at spawn from the install ws), never child-supplied and never on the wire. A sidecar
  installed in ws-A can only ever reach ws-A tools/data. Two-workspace isolation is tested
  through the real gateway (a child installed in ws-B calling `store.query`/`ingest.write` sees
  none of ws-A).
- **Capabilities:** every callback runs the full `authorize_tool` gate against the child's
  `granted` set. Deny is opaque (`403` → `CallError::Denied`). Mandatory deny-test: a sidecar
  calling a verb **its install grant omits** → denied at the host, before any store/bus effect.
- **Placement:** either (symmetric). The client posts to whatever `LB_GATEWAY_URL` the supervisor
  injected — the node fronting this child. No `if cloud`; a node with no gateway simply injects no
  URL and the child's `from_env()` fails loud with `NoGateway` rather than guessing.
- **MCP surface:** **consumes** the existing tool surface; **exposes** no MCP tool — it is a
  *transport*. API-shape (§6.1): request/response only. No CRUD/list/watch added here.
- **Data (SurrealDB):** none added by the transport. The child touches the store *only* through
  host verbs via the callback (the one datastore, one mediated path).
- **Bus (Zenoh):** none added. (Guest-initiated motion is the deferred follow-up.)
- **Sync / authority:** unchanged; a `<ext>.<tool>` callback that targets another node uses the
  existing routed MCP hop from the host side of `call_tool`.
- **Secrets:** the child's `LB_EXT_TOKEN` is a bearer credential — treat as secret, never logged.
  The child never sees the node key; it only holds its own scoped token.
- **SDK/WIT impact:** **none.** This is the native tier — no WIT world, no ABI change. (Contrast
  `host-callback-scope.md`, which was a WIT `@0.2.0` bump for the WASM tier.) The child↔host
  stdio ABI (`lb-supervisor`) is untouched; the callback rides HTTP, out of band.

## Example flow

A native sidecar that writes a record it owns (control-engine's `ce.appliance.add`):

1. The host spawns the `control-engine` child with `LB_EXT_TOKEN` (node-signed, carrying
   `granted ∋ store:ce_appliance:write`) and `LB_GATEWAY_URL=http://127.0.0.1:8080`.
2. At startup the child builds `SidecarClient::from_env()` once (pooled HTTP + resolved config).
3. On a `Call` for `control-engine.appliance.add`, the child validates args, then calls
   `client.call_tool("store.write", json!({ "table":"ce_appliance", "id":"plant-1", "value":{…} }))`.
4. The gateway `POST /mcp/call` verifies the child's token (node key), derives the child's
   principal, authorizes `mcp:store.write:call` **and** the `store:ce_appliance:write` cap
   (workspace-first), and dispatches the generic store-write verb → the record commits in the
   **child's** workspace namespace. `{ "rev":1 }` returns.
5. The child returns `{ "id":"plant-1" }` over stdio; the host surfaces it.
6. **Deny path:** install `control-engine` with a grant that omits `store:ce_appliance:write`;
   step 4 is **denied at the gateway** (`403` → `CallError::Denied`) before any write — the child
   surfaces a loud error, no record lands.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`, all through the **real** gateway +
real supervisor + real store + real caps (no mocks, CLAUDE §9). No fake backend: the child is a
real spawned binary; the callback hits a real `POST /mcp/call`.

- **Capability deny (mandatory).** A sidecar whose grant **omits** the target verb's cap →
  `CallError::Denied` (`403`) at the gateway, before any store/bus effect; assert nothing wrote.
- **Workspace isolation (mandatory).** A child installed in ws-B, calling a read verb via the
  callback, sees **none** of ws-A's seeded data (the ws is host-set in the token, un-spoofable);
  and a child cannot reach across the wall by any body field (ws is never on the wire).
- **Happy round-trip.** A child calls `ingest.write` (or `store.write`, once shipped) via the
  callback against a real seeded store; a separate read verb confirms the row committed.
- **Token verification.** A tampered/foreign-signed bearer → `401` at the gateway (the node-key
  verify path); the shared-key wiring is what makes the legit token pass — a regression test both
  ways.
- **Offline / no-gateway.** No `LB_GATEWAY_URL` injected → `SidecarClient::from_env()` fails with
  `NoGateway` (loud), the child does not guess an address or silently no-op.
- **Hot-reload / restart.** The child holds no durable state; a kill + respawn rebuilds
  `SidecarClient::from_env()` and the callback still works (stateless-extension guarantee).

## Risks & hard problems

- **Sharing the node signing key with the gateway.** The security hinge is that spec.rs (minter)
  and the gateway (verifier) share exactly `Node::key`. Thread it explicitly through
  `boot.rs` → `install_native` → `build_spec`; a mismatch fails **closed** (`401`), never open.
  Test both the pass and the tamper path.
- **`build_spec` signature change ripples.** Adding `key` + `gateway_url` params touches every
  `install_native`/`build_spec` caller (the native install path, its tests). A mechanical but
  wide change — grep every call site; keep the old behavior (no gateway url) as `None`.
- **Token lifetime.** `exp=MAX`, bounded by process lifetime + grant. Acceptable for a supervised
  child (killed on uninstall); note it so a future rotation story knows where to hook.
- **Blast radius of a compromised child.** A native child runs OS-native with a real token — but
  bounded by `granted = requested ∩ admin_approved` and the workspace wall. The intersection +
  host-set ws is the safety property; the deny tests must prove it is real, not displayed.
- **Cross-branch port fidelity.** This exact transport shipped on `ui-ext` (commit `3ff46d3`);
  port the `lb-sidecar-client` crate + the `spec.rs`/`boot.rs`/`install.rs`/`gateway state`
  deltas **cleanly**, dropping the unrelated `thecrew`/`fleet-monitor` churn from that commit. Do
  not fork the client — copy it faithfully and keep it generic.

## Open questions (resolve in-slice)

- **Does ce-v1's gateway `POST /mcp/call` already verify node-signed bearer tokens**, or does the
  `session::authenticate` path need the shared-key wiring ported too? Confirm against the shipped
  route before writing — the answer decides whether this slice touches only spec.rs/boot.rs or
  also the gateway auth path (on `ui-ext` it touched `gateway/src/state.rs` +6).
- **Where does the node key live at install time** on ce-v1 (`Node::key`?), and is it reachable
  from `install_native`'s call chain without a dep inversion? Verify before threading it.
- **One `SidecarClient` per child or per call?** Default: **one, built once at startup** and
  cloned (pooled `reqwest::Client` reuses connections across a poller's many writes) — matches
  the ui-ext shape; only revisit if measured.

## Related

- `scope/extensions/host-callback-scope.md` — the **WASM** in-process dual (`host.call-tool`);
  this is its out-of-process, native-tier twin, sharing the same `call_tool` chokepoint + deny
  semantics.
- `scope/extensions/native-tier-scope.md` — native (Tier-2) supervision + the scoped-identity env
  injection this extends (the co-trust gap it closes).
- `scope/store/store-write-verb-scope.md` — the generic `store.write`/`store.delete` verb this
  transport's first real consumer (control-engine) will call.
- `scope/control-engine/slice-4-appliance-registry-routing.md` — the slice that needs this (and
  `slice-5-write-verbs.md`, whose `outbox.enqueue` also rides it).
- `scope/extensions/ui-federation-scope.md` — the **page** bridge (`POST /mcp/call`); the third
  transport for the same contract.
- `scope/mcp/mcp-scope.md` — the authorize-then-dispatch gate all three transports reuse.
- README `§6.3` (two-tier runtime), `§6.5` (MCP as the contract), `§3` rules 1/4/5/6/7.
- Reference implementation to port: `ui-ext` commit `3ff46d3` — `rust/crates/sidecar-client/*`,
  `rust/crates/host/src/native/spec.rs`, `boot.rs`, `native/install.rs`, `role/gateway/src/state.rs`.
</content>
</invoke>
