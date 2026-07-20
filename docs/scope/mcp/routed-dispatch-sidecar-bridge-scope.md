# mcp scope — routed dispatch over the sidecar/HTTP bridge

Status: scope (the ask). Promotes to `doc-site/content/public/mcp/routed-node-dispatch.md`
(extends it) once shipped.

Routed dispatch (`routed-node-dispatch-scope.md`, BUILT 2026-07-20) gave lb core the ability
to run a tool **on a named node** — but only through the in-process Rust entry
`lb_mcp::call_on_node`. **No HTTP caller can reach it.** The `POST /mcp/call` bridge, the
`lb_host::call_tool` chokepoint it funnels through, and the `SidecarClient` a native sidecar
calls back with all carry only `{tool, args}` — there is no axis to name a target node. So a
native sidecar (ems, and any fleet embedder) can be told *by lb* that a call was `Ambiguous`,
but has **no way to answer** by targeting a node. This scope threads the target node through
those three seams so routed dispatch is usable by the callers that actually need it.

## Why this exists (the gap, verified)

Routed-node-dispatch shipped the engine and the error taxonomy:

- `lb_mcp::call_on_node(registry, bus, principal, ws, tool, input, &NodeId)` — the targeted entry.
- `ToolError::{Ambiguous, NodeUnreachable, NodeTooOld}` + their HTTP mappings (409/503/502),
  wired into `rust/role/gateway/src/routes/mcp.rs` and the sibling routes.

But it stopped at the library boundary. Traced on master (`36ae877d`):

- **`call_on_node` has zero non-test callers.** `grep call_on_node rust/ --include='*.rs'`
  outside `mcp/src/` returns nothing. Every real dispatch path — the `/mcp/call` bridge, the
  agent loop, the gateway routes — goes through `lb_mcp::call` / `call_with_ctx` with
  `target_node = None`.
- **The bridge body has no node field.** `McpCall { tool, args }` in
  `rust/role/gateway/src/routes/mcp.rs`. The handler calls
  `lb_host::call_tool(&gw.node, &principal, ws, &body.tool, &input)` — the untargeted chokepoint.
- **`lb_host::call_tool` has no node parameter.** `rust/crates/host/src/tool_call.rs:208` →
  `call_tool_at_depth` → `dispatch_at_depth` → `lb_mcp::call_with_ctx(..., None)`
  (`tool_call.rs:642`). The node never appears.
- **`SidecarClient::call_tool(tool, input)` has no node axis.** (`lb-ext-sdk`,
  `crates/lb-sidecar-client/src/client.rs:54`.) It POSTs `{tool, args}` to `{gateway_url}/mcp/call`.
  There is no `node` field on the wire and no method that accepts one.

Net: **the routed-dispatch error is reachable but the routed-dispatch success is not.** A sidecar
that receives `409 Ambiguous { candidates }` cannot act on it. For ems specifically this means
gateways-slice-2 (routed remote provisioning) is blocked — ems provisions `modbus.*` exclusively
through `SidecarClient`, so with no node axis on that client it can only ever hit the local node.

## Goals

- A native sidecar can call a host MCP tool **on a named node** through the same
  `SidecarClient` it already uses, e.g. `client.call_tool_on_node(tool, input, &node)`.
- The `POST /mcp/call` bridge accepts an **optional** target node and routes via
  `call_on_node` when present, `call` when absent — one handler, one added axis.
- `lb_host::call_tool` gains an **optional** `target_node`, threaded to
  `call_with_ctx`/`call_on_node` at the bottom, so the bridge and the agent loop share it.
- The three routing errors already mapped (409/503/502) are returned **unchanged** — this scope
  adds the request axis, not new failure modes.
- Backwards compatible: a body with no `node` and a `call_tool` with no node behave exactly as
  today (untargeted resolve, `Ambiguous` if multiply-hosted). No caller is forced to change.

## Non-goals

- **Discovery / auto-targeting.** Which nodes host which extension, and the online/offline
  roster, remain fleet-presence's job (`node-roles/fleet-presence-scope.md`, Findings A/B). This
  scope lets a caller name a node it *already knows*; it does not learn the fleet.
- **`NodeTooOld` emission.** Still gated on the presence `targeted_dispatch` flag (fleet-presence).
  Until then an old node reads as `NodeUnreachable` — unchanged by this scope.
- **The WASM guest `host.call-tool` import.** A local wasm guest gets no routed target (its
  callback identity is node-local by construction — see `call_with_ctx`'s doc). Routing a guest's
  re-entrant call is a separate scope. This scope is the **native-sidecar + page bridge** path.
- **A new capability or grammar.** Addressing is not authorization (routed-node-dispatch §"Naming
  a node is not permission"). No `mcp:<tool>@<node>:call`, no per-node grant. The existing
  `mcp:<ext>.<tool>:call` gates a targeted call identically.
- **Outbox / deferral.** `NodeUnreachable` stays a refusal, never a queue (routed-node-dispatch).

## Intent / approach

One optional axis — `target_node: Option<&NodeId>` — threaded through the three seams that sit
between a sidecar and `call_on_node`, funnelling into the **one** `call_inner` pipeline that
routed-node-dispatch already built. Nothing forks: `None` is today's behaviour, `Some(node)` is
the targeted behaviour, and the authorize→resolve→dispatch ordering (and its deny guarantee) stays
in the single `call_inner` body.

**Wire shape (the one real decision): a body field, not a header, not a tool-name encoding.**

- `{ "tool": "...", "args": {...}, "node": "node:gw-01" }` — `node` optional.
- **Rejected — header (`X-Lb-Node`)**: the bridge already takes tool+args in the body and derives
  workspace from the token; splitting the *target* into a header would put one call's addressing in
  two places, and headers get dropped/rewritten by proxies more readily than a body field.
- **Rejected — encoding the node into the tool name (`modbus@gw-01.device.add`)**: explicitly
  rejected by routed-node-dispatch (grants would multiply by fleet size). The node is call **data**,
  parallel to `args`, never part of the tool identity.

So `node` rides beside `args` as ordinary request data. `NodeId::new` validates it at the bridge
(a bad id is `400 BadInput`, author feedback — the same class as a malformed arg, not a 403).

The three-seam thread:

1. **`SidecarClient`** (`lb-ext-sdk`) — add `call_tool_on_node(tool, input, &NodeId)`. Same POST,
   same token, `node` added to the body. `call_tool` stays as the zero-node convenience (delegates
   with `None`). This is an **SDK boundary change** — flag loudly; ems (and every embedder) picks
   it up on the next `sdk-v*` tag.
2. **`POST /mcp/call`** (`rust/role/gateway/src/routes/mcp.rs`) — `McpCall` gains
   `#[serde(default)] node: Option<String>`. When `Some`, parse `NodeId` (→ `400` on bad id) and
   call the targeted `lb_host` entry; when `None`, today's path. The Ask-gate and workspace-from-token
   logic are untouched (addressing does not widen authority).
3. **`lb_host::call_tool`** (`rust/crates/host/src/tool_call.rs`) — thread
   `target_node: Option<&NodeId>` through `call_tool` → `call_tool_at_depth` → `dispatch_at_depth`,
   selecting `lb_mcp::call_on_node` vs `call_with_ctx` at the bottom. Keep the existing signature
   working via a thin `call_tool` (node = `None`) so the many current callers don't churn — or add
   `call_tool_on_node` alongside. Prefer the additive form to keep the diff to the routed path.

## How it fits the core

- **Tenancy / isolation:** unchanged and load-bearing. The node-qualified bus key is declared
  **per workspace** (routed-node-dispatch §"Workspace isolation"): a targeted call from workspace B
  to a node serving only workspace A has nowhere to land — the key space is the wall. Workspace is
  still derived from the token, never the body. Add a workspace-isolation test for the *targeted*
  path (cross-ws target → `NodeUnreachable`, not a leak).
- **Capabilities:** the same `mcp:<ext>.<tool>:call` gates targeted and untargeted alike.
  Authorization runs **before** the node is looked at — a capless caller is `Denied` identically
  whether or not the named node exists (no fleet-enumeration oracle). Deny-test: revoked
  `mcp:modbus.*:call` + a `node` → `403 Denied`, never `409/503`.
- **Placement:** either. This is generic core/SDK plumbing; no `if cloud {…}`. A solo node ignores
  `node` (or targets itself) exactly as before.
- **MCP surface:** **no new tool, no new capability.** One optional request axis on the existing
  `/mcp/call` bridge and the existing `SidecarClient`. API shape (§6.1): this is not CRUD/list/feed/
  batch — it is a **dispatch-addressing** change to the universal bridge. All four verb shapes N/A.
- **Data (SurrealDB):** none. No records, no tables.
- **Bus (Zenoh):** consumes the existing node-qualified key routed-node-dispatch already declares;
  no new subject. The targeted call is one bus hop to the named node's key (or `NodeUnreachable`).
- **Sync / authority:** none added. `NodeUnreachable` is a refusal, not a deferral (no outbox).
- **Secrets:** none. The callback token is unchanged (node-signed `LB_EXT_TOKEN`); the node id is
  not secret — naming it is not permission to use it.
- **SDK/WIT impact:** **YES — the `SidecarClient` public API changes** (new method).
  Additive, non-breaking (existing `call_tool` stays), but it is the stable plugin boundary — ships
  as a new `sdk-v*` tag that ems/embedders bump to. WIT is untouched (this is the native-sidecar
  HTTP path, not the wasm import).

## Example flow

1. ems `meter.stamp` resolves the meter's gateway → it is `Remote`, bound to `node:gw-01`.
2. ems threads that node into its provisioner and calls
   `client.call_tool_on_node("modbus.device.add", args, &NodeId::new("node:gw-01")?)`.
3. `SidecarClient` POSTs `{tool:"modbus.device.add", args:{…}, node:"node:gw-01"}` to
   `{gateway_url}/mcp/call` with the scoped bearer token.
4. The bridge authenticates the token → principal + ws. Ask-gate (non-run token → skipped).
   `node` present → `NodeId::new` ok → `lb_host::call_tool(..., Some(&node))`.
5. `call_inner` authorizes `mcp:modbus.device.add:call` (before touching the node), resolves the
   node-qualified key **in this ws**, dispatches on `gw-01`. Runs there or fails — never falls back.
6. Success → `{ id }` back to ems. Or: `gw-01` not in this ws → `503 NodeUnreachable`, ems writes
   nothing and surfaces its `gateway_unreachable` refusal. Or: cap revoked → `403 Denied` before
   step 5's resolve, identical whether or not `gw-01` exists.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md` that apply:

- **Capability deny-tests (mandatory):** targeted call with `mcp:<tool>:call` revoked → `Denied`
  (`403`), returned **before** node resolution; identical error whether the named node exists or
  not (no existence oracle). Assert it is not `409/503`.
- **Workspace-isolation (mandatory):** a node serving only ws A, targeted from ws B →
  `NodeUnreachable`, nothing dispatched, no cross-ws signal. The targeted analogue of the existing
  untargeted isolation test.
- **Real store/bus, two nodes (no mocks):** two real nodes via the existing `boot_as(role)` /
  routed test harness (`rust/crates/host/tests/routed_ambiguity_test.rs`, `cross_node_routing_test.rs`
  are the neighbours). A targeted `call_tool_on_node` lands on the named node; 40-call determinism
  check (the misprovisioning bug routed-node-dispatch measured) — 100% on the named node, 0 fallback.
- **Bridge integration:** `POST /mcp/call` with `node` present → routed; absent → untargeted
  (`Ambiguous` if multiply-hosted, unchanged). Bad node id (`gw-*`) → `400 BadInput`, not `403`.
- **SDK unit:** `SidecarClient::call_tool_on_node` puts `node` on the body; `call_tool` omits it
  (byte-for-byte the old body — proves backwards compat).
- **Hot-reload:** N/A (no durable instance state introduced).

## Risks & hard problems

- **The `lb_host::call_tool` fan-out.** `call_tool` has many callers (agent loop, gateway routes,
  reach path). Threading a new required param would churn all of them; the additive
  `call_tool_on_node` (or `Option` with a defaulted thin wrapper) keeps the diff on the routed path.
  Decide once and hold it — a half-threaded node param that silently drops to `None` mid-chain is the
  worst outcome (a targeted call silently runs untargeted, reintroducing the misprovisioning bug).
- **Silent-fallback regression.** The entire point of routed-node-dispatch is *no fallback*. Any
  code path where a `Some(node)` degrades to `None` (a parse slip, a default, a lost `Option` in the
  thread) reintroduces "landed on the wrong box, returned success." The two-node determinism test is
  the guard; it must run in CI, not just locally.
- **SDK version skew.** ems bumps `sdk-v*` for `call_tool_on_node` **and** the `node-v*` tag for the
  bridge/host change together — the sidecar client and the host route are two repos. A sidecar
  sending `node` to an **old** host: the old bridge `#[serde(default)]`-ignores an unknown field and
  runs untargeted — i.e. silently local. So the ems bump must move **both** pins, and the honest
  behaviour on an old host is "untargeted" — call it out so a partial bump isn't mistaken for routing.
- **`400 vs 403` for a bad node id.** A malformed `node` is author feedback (`400 BadInput`), the
  same call routed-node-dispatch already made for a `{key}`-vs-`{id}` arg typo. It must **not**
  collapse to `403` (that reads as a capability denial and hides the typo).

## Open questions

- **OQ1 — additive `call_tool_on_node` vs `Option` param on `call_tool`?** Recommend additive
  (`call_tool_on_node` in both `lb_host` and `SidecarClient`) to keep the fan-out diff minimal and
  make the routed path greppable. Confirm at implementation.
- **OQ2 — does the agent loop want the same axis now?** The agent dispatch path
  (`rust/crates/host/src/agent/`) is the other caller that hits `Ambiguous`. Out of scope to *wire*
  here, but the `lb_host` signature should be shaped so the agent loop can adopt it without another
  signature change. Confirm the signature is agent-ready.
- **OQ3 — reply shape for `Ambiguous` over the bridge.** Routed-node-dispatch returns candidates as
  data in the `ToolError`. Confirm the `409` body a sidecar receives carries the candidate node ids
  as parseable JSON (not just prose), so a future auto-target caller can read them. If it currently
  only stringifies, that is a small addition to fold in here.

## Related

- **Tracking issues:** this scope = [lb#85](https://github.com/NubeDev/lb/issues/85) (seams 2+3) ·
  seam 1 = [lb-ext-sdk#4](https://github.com/NubeDev/lb-ext-sdk/issues/4) · consumer =
  [ems#9](https://github.com/NubeIO/ems/issues/9) · engine = [lb#81](https://github.com/NubeDev/lb/issues/81)
  · discovery = [lb#82](https://github.com/NubeDev/lb/issues/82).
- `mcp/routed-node-dispatch-scope.md` — the engine this exposes (BUILT 2026-07-20); its public doc
  `doc-site/content/public/mcp/routed-node-dispatch.md` §"What is not here yet" names this gap
  ("Discovery … supplied by explicit wiring") — this scope is the caller-facing half.
- `node-roles/fleet-presence-scope.md` — the still-owed discovery + `targeted_dispatch` flag
  (Findings A/B); complements this (this = *name a node you know*, that = *learn the fleet*).
- `mcp/ems-provisioning-verb-shapes-scope.md` — the `modbus.*` verb shapes ems routes through
  `SidecarClient`; the first real consumer of this axis.
- ems `docs/scope/gateways/gateways-scope.md` + `docs/sessions/gateways/gateways-slice2-handover.md`
  — the blocked downstream. Slice 2 cannot proceed until this ships in a `node-v*` + `sdk-v*` tag.
- SDK boundary: `lb-ext-sdk` `crates/lb-sidecar-client/src/client.rs` (the client to extend).
