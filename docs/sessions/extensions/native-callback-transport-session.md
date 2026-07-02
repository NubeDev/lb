# Session — the native-sidecar → host MCP callback transport

- **Scope:** [`scope/extensions/native-callback-transport-scope.md`](../../scope/extensions/native-callback-transport-scope.md)
- **Resolves:** the deferred "child→host callback transport" open question in
  [`scope/extensions/native-tier-scope.md`](../../scope/extensions/native-tier-scope.md).
- **Stage:** S7 follow-up (native tier). The native-tier slice proved supervision with a child
  that needed only the control line; this wires the **other direction** — a supervised OS child
  calling *back* into the host's MCP surface — for real.
- **Status:** shipped. Backend green (3/3 real-gateway callback tests pass).
- **Date:** 2026-07-02.

## The gap, restated

The native-tier slice shipped supervision (spawn / handshake / health / cooperative-stop /
restart-on-crash) and injected a scoped identity into the child (`LB_EXT_WS` / `LB_EXT_ID` /
`LB_EXT_TOKEN`). But it **deferred the callback transport**: a sidecar could not actually reach a
host MCP tool, because the two ends were never wired to a real node. Worse, the injected
`LB_EXT_TOKEN` was a **placeholder**: `crates/host/src/native/spec.rs` minted it with
`SigningKey::generate()` — a **throwaway** key created and dropped on the spot. Nothing anywhere
held that key, so no verifier could ever check the token. The native-tier scope flagged this
honestly as a "co-trust posture, deferred" hack: the child carried a shape-correct JWT that no
gateway could validate.

That placeholder is the whole reason the callback was deferred. The wasm guest reaches host tools
**in-process** through the `host.call-tool` WIT bridge (the host is right there, identity is set in
`HostState`). A native child is **out of process** — its only route back is HTTP to the gateway,
and the gateway must *verify* the child's token before it authorizes anything. A throwaway signing
key makes that impossible. So the callback couldn't be built without first making the child token
**genuinely verifiable**.

## The key decision — one signing identity on the `Node`

**Move the node's signing key onto `Node`, so the token the native tier mints is the same identity
the gateway already verifies.** Before, the gateway owned its login/authenticate signing key
privately in `Gateway`, and the native tier minted with a *different* (throwaway) key — two
identities, one of them un-verifiable by design.

- `crates/host/src/boot.rs`: `Node` gains `key: Mutex<Arc<SigningKey>>` with a `Node::key()`
  accessor and a `Node::install_key()` installer — mirroring the existing
  `runtimes` / `install_runtimes` pattern already on `Node` (a slot the role fills at build time,
  not a compile-time branch — symmetric nodes, rule 1).
- `role/gateway/src/state.rs` (`Gateway::build`): the gateway now calls `node.install_key(key)`, so
  the **one** key it uses for login/authenticate is the **same** key the native minter reads. There
  is now a single signing identity for the whole node.
- `crates/host/src/native/spec.rs::build_spec` takes `&SigningKey` (+ an optional `gateway_url`)
  instead of generating one; `crates/host/src/native/install.rs` passes `node.key()` and injects
  `LB_GATEWAY_URL` (read from env) into the child's environment alongside the existing
  `LB_EXT_*` vars.

The result: a child's `LB_EXT_TOKEN` is now a real node-signed JWT the gateway verifies on the
callback. No throwaway, no co-trust hack. The workspace and grant come from *inside* that token,
so they are un-spoofable — a child cannot claim a workspace it wasn't minted for.

**Alternative considered & rejected — keep the throwaway co-trust token and wire the callback
around it.** Tempting because it touches less: leave `spec.rs` generating its own key, and have the
gateway "trust the loopback" or accept any well-formed token from localhost. Rejected on two
grounds. (1) It can *never* be verified by a real gateway — the signing key is discarded the
instant the token is minted, so "verify" would have to mean "don't verify," which is a hole, not a
transport. (2) It would force a **fake verifier** (a localhost-trusts-everyone path) to make the
tests pass — exactly the banned "look done while the real path is unbuilt" per CLAUDE rule 9 /
testing §0. The honest fix is cheap and structural: give the node one key. So we did.

## What changed, file by file

- `crates/host/src/boot.rs` — `Node.key: Mutex<Arc<SigningKey>>`, `Node::key()`, `Node::install_key()`.
- `role/gateway/src/state.rs` — `Gateway::build` installs the gateway's signing key onto the node
  (`node.install_key(key)`), unifying the identity.
- `crates/host/src/native/spec.rs` — `build_spec` takes `&SigningKey` + `Option<gateway_url>`;
  mints `LB_EXT_TOKEN` with the node key (no more `SigningKey::generate()`).
- `crates/host/src/native/install.rs` — passes `node.key()` into `build_spec`; injects
  `LB_GATEWAY_URL` from env into the child.
- **New crate `crates/sidecar-client`** (package `lb-sidecar-client`) — the generic out-of-process
  transport:
  - `error.rs` — `CallError` enum: **`Denied`** (the capability/workspace 403 — a first-class,
    distinct variant, never a panic, never conflated with transport error), `NoGateway`, `NoToken`,
    `Http { status, message }`, `Transport`, `Decode`.
  - `config.rs` — `Config` reads `LB_EXT_WS` / `LB_EXT_ID` / `LB_EXT_TOKEN` / `LB_GATEWAY_URL` from
    the env (the vars the supervisor injects), or `Config::new(...)` for tests.
  - `client.rs` — `SidecarClient::from_env()` / `with_config(...)`, and
    `async call_tool(tool, input: Value) -> Result<Value, CallError>` which POSTs `{tool, args}` to
    `{gateway}/mcp/call` with `Authorization: Bearer <token>`, mapping HTTP 403 → `CallError::Denied`.
- **Proof wiring** — `extensions/fleet-monitor`: a new `fleet.probe` tool (`src/call.rs`, now
  async; `extension.toml` adds the tool entry) calls back into the host for `series.find` via
  `lb-sidecar-client`, proving the child→host path end to end. fleet-monitor's manifest already
  grants `mcp:series.find:call`.

## The symmetry (rule 7)

`lb-sidecar-client` is the **out-of-process peer of the wasm guest's in-process `host.call-tool`
bridge**. Both are transports for the *one* MCP contract:

- wasm guest → `host.call-tool` (WIT import) → `lb_host::call_tool` (in-process, `HostState`
  identity);
- native child → `lb-sidecar-client` → `POST /mcp/call` (HTTP, `Bearer` token identity) →
  the same `call_tool` chokepoint at the gateway.

Both are denied identically by the host gate (workspace-first, then `mcp:<tool>:call`). Nothing
extension-specific lives in `lb-sidecar-client` — ros / mqtt / control-engine all reuse it as-is.

## Test strategy — a real gateway over real HTTP

`reqwest` needs a **real socket**, so an in-process `oneshot`/`ServiceExt` call against the axum
router would not exercise the transport we ship. So the tests stand up the real thing:
`role/gateway/tests/native_callback_test.rs` boots a **real `Node`** + a **real axum gateway on a
real TCP port**, mints a child token with the node key *exactly as `native/spec.rs` does*, and
drives the **real `lb-sidecar-client` over real `reqwest` HTTP** at that port. Series are made
findable via **real tag edges** (`tags_add`), because `series.find` discovers over the tag graph,
not over sample labels — seeding real records into the real store, per rule 9.

The three mandatory-category tests (all pass):

```
running 3 tests
test granted_sidecar_callback_reaches_series_find ... ok
test ungranted_sidecar_callback_is_denied_and_leaks_nothing ... ok
test ws_b_callback_cannot_see_ws_a_series ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- **happy round-trip** — `granted_sidecar_callback_reaches_series_find`: a granted child token
  round-trips over HTTP and returns a real seeded series.
- **capability deny** — `ungranted_sidecar_callback_is_denied_and_leaks_nothing`: a token *without*
  `mcp:series.find:call` gets `CallError::Denied` (403) and **no data leaks** — the deny is
  first-class, not a transport error.
- **workspace isolation** — `ws_b_callback_cannot_see_ws_a_series`: a ws-B token sees **none** of
  ws-A's seeded series. The workspace comes from inside the token (the load-bearing consequence of
  the one-key fix — un-spoofable).

## What this unblocks

With a verifiable child token and a generic transport, a native sidecar is now a first-class MCP
caller. This unblocks the **ROS driver** (its poller → `ingest.write`, its `point.write` →
`outbox.enqueue`) and, later, the **mqtt bridge** and **control-engine** — all of which reach the
platform through `lb-sidecar-client` with no bespoke socket and no new trust surface.

## Files

- Host: `crates/host/src/boot.rs`, `crates/host/src/native/spec.rs`, `crates/host/src/native/install.rs`.
- Gateway: `role/gateway/src/state.rs`.
- New crate: `crates/sidecar-client/src/{error.rs, config.rs, client.rs, lib.rs}` + `Cargo.toml`
  (package `lb-sidecar-client`).
- Ext: `extensions/fleet-monitor/src/call.rs`, `extensions/fleet-monitor/extension.toml`
  (`fleet.probe`).
- Tests: `role/gateway/tests/native_callback_test.rs` (+3).
