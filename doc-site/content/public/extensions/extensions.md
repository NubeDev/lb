# Extensions

TODO — filled as the extensions surface ships. Covers: the two tiers (WASM / native), the
`extension.toml` manifest, the signed-`Artifact` publish path, the devkit / Extension Studio,
and (per `docs/scope/extensions/ext-out-of-tree-scope.md`) the out-of-tree SDKs
(`lb-sdk`, `lb-ext-native`, `@nube/ext-ui-sdk`) and the `lb-ext` CLI.

See `docs/scope/extensions/` for the asks and `docs/public/extensions/dev-flow.md` (if present)
for the current build → pack → publish chain.

## Dev flow: packaging & publishing (build → pack → publish)

The bridge between an extension's `build.sh` and the gateway's `POST /extensions` is **`lb-pack`**,
the artifact packager. It and its library `lb-devkit` are **published lb crates, consumed by git
tag** (the same model as embedding the core via `lb-node`) — an embedder never copies the tool or
re-implements the signing. `lb-pack` shares the exact digest + Ed25519 idiom the node verifies with
(`lb-devkit` → `lb-registry`), so a packed artifact verifies **by construction**.

```sh
# 1. Install once, pinned to the SAME lb tag you embed (version alignment = format alignment):
cargo install --git https://github.com/NubeDev/lb --tag node-v0.3.3 lb-pack

# 2. Build your extension, then pack it (generates the publisher key on first run):
lb-pack myext/extension.toml myext/target/wasm32-wasip2/release/myext.wasm \
        keys/dev.key --key-id my-publisher --out artifacts/myext.json

# 3. First run only — print the trust line and start the node with it:
lb-pack pubkey keys/dev.key --key-id my-publisher   # → my-publisher=<hexpubkey>
#    LB_TRUSTED_PUBKEYS="my-publisher=<hexpubkey>" …

# 4. Publish with a session token carrying ext.publish:
curl -X POST "$NODE/extensions" -H "Authorization: Bearer $TOKEN" \
     -H 'content-type: application/json' --data @artifacts/myext.json   # → 204
```

**The publisher signing key is sensitive.** It is the identity a node trusts via
`LB_TRUSTED_PUBKEYS` — never commit the key file; only the trust *line* (the public half) is
shared. Signing is local and grants nothing: trust is established node-side, an artifact from an
untrusted key is rejected at verify, and uploading still requires the `ext.publish` capability.

For programmatic packing (an embedder's own tooling, the future `lb-ext` CLI), depend on the
stable `lb-devkit` core at the same tag:

```toml
lb-devkit = { git = "https://github.com/NubeDev/lb", tag = "node-v0.3.3", default-features = false }
```

The stable surface is `sign_artifact`, `load_or_create_key`, `publisher_trust_line`,
`LoadedPublisherKey`, and the signed `Artifact` type; the default-on `devkit-full` feature
(scaffold/build/inspect/toolchains) is node-side machinery, not an embedder contract. Full how-to:
`docs/skills/lb-pack/SKILL.md`.

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

