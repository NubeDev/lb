# MCP scope

Status: scope. Defines the MCP tool layer (README §6.5) and the caller contract for S1.
Promotes to `public/mcp/` once the spine proves a routed, capability-checked tool call.

> Read with: `../../README.md` §6.5 (MCP/tool layer), §3.7 (MCP is the universal contract),
> `../auth-caps/auth-caps-scope.md` (every call is caps-checked first),
> `../crate-layout/crate-layout-scope.md` (the WIT `call-tool` export).

---

## Goal

Every node runs an MCP server (rmcp) exposing the tools of the extensions hosted on it. A
tool call is **capability-checked workspace-first, then dispatched** to the hosting
extension. In S1 (solo node) there is no cross-node routing yet, but the call path is shaped
so that adding Zenoh-queryable routing in S3 is a drop-in, not a redesign.

## Non-goals (S1)

- Cross-node tool routing over Zenoh queryables (README §6.5) — S3. S1 dispatches locally.
- The AI-agent and UI callers (§6.5 names three callers) — S1 exercises the path with a
  direct/test caller; agent + UI arrive at S5/S2.
- Tool *discovery*/catalog UI — S1 has a fixed in-process registry of loaded extensions.

---

## The contract

One MCP tool name is `<extension>.<tool>` (matching the `mcp:` capability resource, auth-caps
scope). A call is:

```
call(principal, "<ext>.<tool>", json_input) -> result<json_output, ToolError>
```

The server pipeline, one step per file (FILE-LAYOUT):

```
mcp/src/call/
  mod.rs        ← orchestrates the four phases, ≤50 lines
  resolve.rs    ← name "<ext>.<tool>" -> hosting extension + tool descriptor
  authorize.rs  ← caps::check(principal, mcp:<ext>.<tool>:call) — DENY here, before dispatch
  dispatch.rs   ← invoke the extension's WIT `call-tool` (local in S1; routed in S3)
  error.rs      ← ToolError (NotFound | Denied | ExtensionError | BadInput)
```

`authorize.rs` calls the **same** `caps::check` chokepoint as store/bus/secret — MCP is not a
special case. Workspace isolation is gate 1 there; the missing-grant deny is gate 2.

**Why authorize before resolve-dispatch matters:** a denied call must not leak whether the
tool exists. `resolve` runs first only to map the name; `authorize` returns `Denied` with no
tool-existence signal in the error to an unauthorized caller. (Tracked as a test.)

## The single S1 tool: `hello.echo`

The trivial WASM extension (`rust/extensions/hello`) exports one tool via the WIT
`call-tool`: `hello.echo` takes `{ "msg": string }` and returns `{ "echo": string }`. It is
the smallest thing that proves the whole spine: a caller → MCP → caps → WIT → WASM → back.

## How it fits the core

- **MCP is the universal contract (§3.7):** the same `call(principal, name, input)` shape
  serves every caller; S1 has one, but the signature is caller-agnostic.
- **Capability-first:** `authorize.rs` is the only gate to dispatch; remove it and there is no
  other path — there is no "internal" un-checked dispatch.
- **Routing-ready:** `dispatch.rs` is the seam. Local now; in S3 it asks "is this ext local?
    else route via Zenoh queryable" — the callers and `authorize` are unchanged.

## Testing plan (mandatory categories apply)

- `mcp/tests/echo_with_grant_test.rs` — call `hello.echo` **with** `mcp:hello.echo:call`
  succeeds and round-trips the payload. (exit-gate happy path)
- `mcp/tests/echo_without_grant_test.rs` — **mandatory deny:** same call without the grant →
  `ToolError::Denied`, and the error does not reveal the tool's existence.
- `mcp/tests/isolation_test.rs` — **mandatory isolation:** a workspace-B principal cannot call
  a tool acting on workspace-A data / is denied at gate 1.

These three are exactly the S1 exit gate (STAGES.md S1).

## What shipped in S3 (cross-node routing)

- `dispatch.rs` is now the real seam: a `Remote` target routes over the bus queryable
  (`mcp/{ext}/call`) to the hosting node; a `Local` target calls the instance. `lb_mcp::serve_call`
  + `lb_host::serve_ext` are the serving side. Callers and `authorize` are unchanged from S1;
  `caps::check` runs on the **calling** node, workspace-first. `call` gained `bus` + `ws`.
- The serving node does **not** re-authorize — the calling node did, and the workspace-scoped
  queryable key means a routed request can only target the caller's own workspace.
- Proven by `host/cross_node_routing_test` (routed call + deny + ws-isolation across two nodes).

## Open questions

- Tool input/output schema format: raw JSON; adopt JSON-Schema snapshots as the **contract test**
  once there's a second tool. Golden-file location TBD with the WIT snapshots.
- Streaming tool results (for AI/gateway, §6.14) — out of scope until S5; a deliberate WIT bump.
- Routing tie-breaks when **two nodes host the same extension** — **now scoped in
  [`routed-node-dispatch-scope.md`](routed-node-dispatch-scope.md)** (issue #81). Note the severity
  is worse than a missing tie-break: every host answers the same key and `lb_bus::query` keeps the
  first reply, so today's behaviour is a *silent nondeterministic wrong node*, not an error. That
  scope puts the target node on the bus key and makes the untargeted multi-host case an explicit
  `Ambiguous` refusal. Prerequisite: fleet-presence's `NodeId` (unbuilt).
- **Serve-side authorization** when a hub-hosted extension touches *hub-authoritative* data —
  would need the principal/grant on the wire (token-on-the-bus). Sufficient today because routed
  tools (hello) touch no hub-owned data and the workspace wall holds on the queryable key.
- **Remote-extension discovery:** S3 registers remote extensions explicitly
  (`register_remote_extension`); a discovery/registry flow (which node hosts what) lands S4/S7.
