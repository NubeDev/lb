# Core (shipped — S1 spine)

What the platform core actually does today. Scope: `../../scope/core/core-scope.md`. Session:
`../../sessions/core/s0-s1-spine-session.md`.

## The spine

A solo node boots embedded SurrealDB + an embedded Zenoh peer + the wasmtime component
runtime, loads a WASM extension via its manifest, and answers capability-checked MCP tool
calls:

```
caller → mcp → caps::check (ws-gate, cap-gate) → runtime → WASM ext (hello.echo) → back
              ▲                                     │
        auth (principal)      store (SurrealDB)   bus (Zenoh)   — embedded, in-process
```

`Node::boot()` assembles store + bus + engine + the MCP registry. `load_extension(node,
manifest, wasm, admin_approved)` parses the manifest, checks the WIT world major, grants
`requested ∩ admin_approved`, instantiates the component, and registers its tools.

## Principles in force

- **Symmetric nodes**: roles are config; no `if cloud` in `crates/`. S1 runs solo.
- **One datastore / state vs motion**: SurrealDB holds state, Zenoh moves messages; separate
  crates, never substituted.
- **Capability-first**: `mcp` authorizes via `caps::check` before any dispatch — no other path.
- **Workspace is the hard wall**: workspace = SurrealDB namespace + bus key prefix; isolation
  is checked first and is structural at the store/bus layers.
- **Stateless extensions**: `hello` holds no durable state (hot-reload-safe; proven at S2).

## Proven (S1 exit gate)

A tool call routed through MCP succeeds *with* the grant and is refused *without* it; a second
workspace cannot see the first's data. 35 tests pass, including the mandatory capability-deny
and workspace-isolation categories. See `auth-caps.md`, `mcp.md`, `crate-layout.md`.

## Next

S2 brings messaging (bus pub/sub + presence) and the React/Tauri UI, and proves hot-reload.
