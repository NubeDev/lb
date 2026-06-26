# MCP (shipped — S1 + S3 cross-node routing)

The tool layer (README §6.5). Scope: `../../scope/mcp/mcp-scope.md`. Sessions:
`../../sessions/core/s0-s1-spine-session.md` (S1), `../../sessions/sync/multi-node-sync-session.md`
(S3 routing).

## The contract

One tool name is `<extension>.<tool>` (matching the `mcp:` capability resource). A call is:

```
call(registry, bus, principal, ws, "<ext>.<tool>", json_input) -> Result<json_output, ToolError>
```

`ToolError` is `Denied | NotFound | Extension(..) | BadInput(..)`. `Denied` carries no detail.
(`bus` + `ws` carry the routed path; for a purely local call they are unused beyond the workspace.)

## The pipeline (one phase per file)

```
call → authorize → resolve → dispatch
```

- **authorize** runs the shared `caps::check` for `mcp:<ext>.<tool>:call` (workspace-first,
  then capability), on the **calling** node. It runs **before** resolve, so a denied caller cannot
  distinguish "not allowed" from "no such tool" — both yield the same opaque `Denied`. A routed
  call is authorized here too: the remote node never sees an unauthorized call.
- **resolve** maps `<ext>.<tool>` to a `Target` — `Local` (a live instance) or `Remote` (hosted on
  another node). Only reached once authorized.
- **dispatch** calls the local instance's WIT `tool.call`, OR — for a `Remote` target — **routes
  over a Zenoh queryable** (`mcp/{ext}/call`, workspace-scoped) to the hosting node, which runs its
  own local dispatch and replies. The call site is identical whether the extension is local or
  remote — that is the seam S1 shaped and S3 made real.

## Cross-node routing (S3)

- The registry holds a `Target` per extension; `register_remote_extension` adds a routing entry.
- `lb_host::serve_ext` declares the serving queryable (`ws/*/mcp/{ext}/call`) and answers routed
  calls by running local dispatch (`lb_mcp::serve_call`). The serving node does **not** re-authorize
  — the calling node already did, workspace-first, and the workspace-scoped queryable key means a
  routed request can only ever target the caller's own workspace.
- Proven by `host/cross_node_routing_test`: a call on the edge routes to hello on the hub and
  returns; an ungranted call is denied on the edge (never routes); a ws-B principal cannot route
  into ws-A.

## S1 surface

One extension (`hello`) with one tool (`hello.echo`): `{ "msg": string } -> { "echo": string }`,
proving caller → MCP → caps → WIT → WASM → back end to end (`host/tests/spine_test`).

## Deferred

The agent + UI callers (S5; the UI reaches tools via the gateway at S3), JSON-Schema contract
snapshots, streaming results (S5, a deliberate WIT bump), serve-side authorization for
hub-authoritative data, and multi-host tie-break when two nodes host the same extension.
