# MCP (shipped — S1)

The tool layer (README §6.5). Scope: `../../scope/mcp/mcp-scope.md`. Session:
`../../sessions/core/s0-s1-spine-session.md`.

## The contract

One tool name is `<extension>.<tool>` (matching the `mcp:` capability resource). A call is:

```
call(registry, principal, ws, "<ext>.<tool>", json_input) -> Result<json_output, ToolError>
```

`ToolError` is `Denied | NotFound | Extension(..) | BadInput(..)`. `Denied` carries no detail.

## The pipeline (one phase per file)

```
call → authorize → resolve → dispatch
```

- **authorize** runs the shared `caps::check` for `mcp:<ext>.<tool>:call` (workspace-first,
  then capability). It runs **before** resolve, so a denied caller cannot distinguish "not
  allowed" from "no such tool" — both yield the same opaque `Denied`.
- **resolve** maps `<ext>.<tool>` to the hosting extension (only reached once authorized).
- **dispatch** invokes the extension's WIT `tool.call` (local in S1; the seam where S3 adds
  Zenoh-queryable routing to a remote node, with callers and authorize unchanged).

## S1 surface

One extension (`hello`) with one tool (`hello.echo`): `{ "msg": string } -> { "echo": string }`,
proving caller → MCP → caps → WIT → WASM → back end to end (`host/tests/spine_test`).

## Deferred

Cross-node routing (S3), the agent + UI callers (S5/S2), JSON-Schema contract snapshots, and
streaming results (S5, a deliberate WIT bump).
