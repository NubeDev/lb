# Extensions

TODO — filled as the extensions surface ships. Covers: the two tiers (WASM / native), the
`extension.toml` manifest, the signed-`Artifact` publish path, the devkit / Extension Studio,
and (per `docs/scope/extensions/ext-out-of-tree-scope.md`) the out-of-tree SDKs
(`lb-sdk`, `lb-ext-native`, `@nube/ext-ui-sdk`) and the `lb-ext` CLI.

See `docs/scope/extensions/` for the asks and `docs/public/extensions/dev-flow.md` (if present)
for the current build → pack → publish chain.

## Native extensions: calling host MCP verbs back (host-callback)

A native (Tier-2) extension is a subprocess the host supervises over stdio. The `lb-ext-native` SDK
crate gives it the **host→child** direction (the host dispatches your tools to you). To go the other
way — **call a host MCP verb back into the core** — use the host-callback client, re-exported from
the same crate (published as `lb-sidecar-client`, `sdk-v0.3.0`+):

```rust
use lb_ext_native::{SidecarClient, CallError};
use serde_json::json;

let host = SidecarClient::from_env()?;                 // supervisor-injected identity from env
let out = host.call_tool("<verb>", json!({ /* args */ })).await?;   // e.g. "ingest.write", "authz.check_scoped"
```

- **One dependency, both directions.** Pin only `lb-ext-native`; the callback client rides along.
- **Verb-agnostic.** `call_tool(name, args)` reaches whatever host verb your manifest was **granted**
  (`granted = requested ∩ admin_approved`). Nothing is special-cased.
- **Authenticated + gated as any caller.** It POSTs `{tool, args}` to the gateway's `/mcp/call` with
  your injected node-signed token; the host runs the full workspace-first capability gate. The
  workspace is the **token's**, never the request body — a callback can only ever reach its own
  workspace's data.
- **Deny is typed, never a panic.** An ungranted verb (or a cross-workspace reach) returns
  `Err(CallError::Denied)` — distinct from transport/other-HTTP errors.

This is the out-of-process peer of the wasm guest's in-process `host.call-tool` bridge: two
transports, one MCP contract, one gate.

