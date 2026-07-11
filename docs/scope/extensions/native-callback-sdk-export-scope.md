# Native callback SDK export scope — publish the host-callback client through the SDK

Status: **SHIPPED** (2026-07-11). Built per this scope — see
[`sessions/extensions/native-callback-sdk-export-session.md`](../../sessions/extensions/native-callback-sdk-export-session.md).

> Read with: [`native-callback-transport-scope.md`](native-callback-transport-scope.md) (the transport
> this scope *publishes* — the mechanism is unchanged, only its distribution), [`ext-out-of-tree-scope.md`](ext-out-of-tree-scope.md)
> (the SDK-owns-the-contract cutover this extends), README §3 rule 7/10.

## The gap

The native-callback transport (`SidecarClient`, the out-of-process peer of the wasm guest's
`host.call-tool` bridge) shipped in 2026-07 as an **in-tree `lb` crate** (`rust/crates/sidecar-client`),
consumed by in-tree extensions (`ros`, `fleet-monitor`, `control-engine`) via a workspace **path**
dependency. That was fine while every native extension lived in the `lb` tree.

It is **not** fine for an **out-of-tree** native extension. After the SDK cutover, an out-of-tree
extension pins the platform contract by **git tag** from `NubeDev/lb-ext-sdk` (`lb-ext-native`,
`lb-sdk`). The host-callback client was never published there — so a native extension had the
host→child control wire (`lb-ext-native`) but **no way to call back** into the host's MCP surface. A
downstream product hit this exactly: its native authz chokepoint needed to call the already-shipped
generic verbs `authz.check_scoped` / `authz.scope_filter` from a native-tier extension, and the SDK
exposed no host-callback client. The wasm (WIT) tier could reverse-call; the native tier could not —
a distribution gap, not a design gap.

## Goals

- **Publish the callback client through the SDK.** `lb-sidecar-client` becomes a first-class crate in
  `NubeDev/lb-ext-sdk`, fetchable by git tag like `lb-sdk`/`lb-ext-native`.
- **One dependency for a native extension.** Re-export `SidecarClient` / `Config` / `CallError` from
  `lb-ext-native`, so an extension carries a single platform crate for **both** directions of the wire
  (host→child dispatch and child→host callback). The `lb-ext-native` manifest + `README` already
  *claimed* it provided "the host-callback client"; this makes that true.
- **`lb` consumes it back.** The `lb` monorepo drops its in-tree path crate and pins the SDK git tag —
  one source of truth, the same posture it already holds for `lb-sdk`.
- **Nothing else changes.** Verb-agnostic, product-agnostic (rule 10). No new host endpoint, no new
  MCP verb, no WIT change, no capability-grammar change.

## Non-goals

- **No behavior change to the transport.** The client is byte-for-byte the shipped one (only its
  crate home + doc comments move). The gateway `/mcp/call` endpoint is untouched.
- **No new verb surface.** This publishes a *client*; the verbs it reaches (`authz.check_scoped`,
  `ingest.write`, any `<ext>.<tool>`) already exist and are unchanged.
- **No special-casing of any verb or product.** `call_tool(name, args)` is verb-agnostic; the host's
  capability + workspace gate is the only thing that decides what a child may reach.

## How it fits the core

- **Tenancy / isolation:** unchanged — the callback's workspace is the one inside the child's token,
  verified by the gateway, never client-supplied. Proven end to end by
  `role/gateway/tests/native_callback_test.rs::ws_b_callback_cannot_see_ws_a_series`.
- **Capabilities:** unchanged — every callback runs the full gate; an ungranted verb → `CallError::Denied`
  (`role/gateway/tests/native_callback_test.rs::ungranted_sidecar_callback_is_denied_and_leaks_nothing`).
- **MCP surface:** consumes the existing surface over the existing endpoint; exposes nothing new.
- **SDK/WIT impact — flagged:** additive. A **new crate** in the SDK workspace + a re-export from
  `lb-ext-native`. No WIT world change, no `WORLD_MAJOR`/`PROTOCOL_MAJOR` bump (the wire is identical).
  Ships as **`sdk-v0.3.0`** (minor: additive surface). `lb` bumps its SDK pin and re-tags **`node-v0.3.0`**
  because its dependency source moved (path → git), even though no host code changed.
- **No mocks:** the SDK-side test drives the real `SidecarClient` against a real `tokio` TCP server
  speaking the `/mcp/call` shape; the lb-side end-to-end test runs a real gateway + real gate.

## Example flow

1. An out-of-tree native extension pins `lb-ext-native = { git = ".../lb-ext-sdk", tag = "sdk-v0.3.0" }`.
2. Inside a tool body it calls `SidecarClient::from_env()?.call_tool("authz.check_scoped", args).await` —
   the supervisor-injected token authenticates it; the host authorizes `mcp:authz.check_scoped:call`.
3. Granted → the JSON result. Ungranted → `Err(CallError::Denied)` (never a panic).

## Testing plan

- **SDK repo** (`lb-ext-native/tests/host_callback_test.rs`): real `reqwest` → real TCP server;
  asserts the wire contract (`POST /mcp/call`, `Bearer`, `{tool,args}`, no `ws` in body) + round-trip,
  and a real `403 → CallError::Denied` (mandatory deny).
- **`lb` repo** (`role/gateway/tests/native_callback_test.rs`, unchanged): now consumes the git-tagged
  crate; the three real-gateway tests (granted round-trip, capability-deny, workspace-isolation) are the
  mandatory categories, green after the repoint.

## Open questions

- ✅ Owning crate: a **separate `lb-sidecar-client`** crate (not merged into `lb-ext-native`) — the
  HTTP transport is a distinct responsibility from the stdio wire (FILE-LAYOUT), mirroring lb's own
  `lb-supervisor` (wire) vs `lb-sidecar-client` (callback) split. `lb-ext-native` **re-exports** it so
  consumers still carry one dependency.
- ✅ Latent bug found + fixed: `lb-ext-native`'s `serve` test harness dropped only the write half of the
  host duplex, so the child never observed EOF and `server.await` hung whenever a non-shutdown test ran
  without a later shutdown test masking it. Now drops both halves. See
  [`debugging/extensions/native-serve-test-eof-hang.md`](../../debugging/extensions/native-serve-test-eof-hang.md).

## Related

`native-callback-transport-scope.md` · `ext-out-of-tree-scope.md` · `ext-sdk-scope.md` ·
`auth-caps/entity-scoped-grants-scope.md` (the motivating downstream consumer of `authz.check_scoped`) ·
README §3 rule 7/10.
