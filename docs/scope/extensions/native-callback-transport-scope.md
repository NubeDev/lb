# Native callback transport scope — a native sidecar calls host MCP tools

Status: **SHIPPED** (2026-07-02). Built per this scope — see
[`sessions/extensions/native-callback-transport-session.md`](../../sessions/extensions/native-callback-transport-session.md).
This resolves the deferred **"child→host callback transport"** open question in
[`native-tier-scope.md`](native-tier-scope.md) (now marked RESOLVED there).

The native-tier slice shipped supervision + a scoped injected identity, but proved its exit gate
with a child that used **only the control line** — it deliberately deferred the transport a sidecar
uses to call *back* into host capabilities. It also left the injected `LB_EXT_TOKEN` as a
placeholder minted with a **throwaway** `SigningKey::generate()` (a "co-trust posture, deferred"
hack): shape-correct, but un-verifiable by any real gateway. This scope closes both gaps at once —
it makes the child token a genuine node-signed JWT and ships the generic HTTP transport a sidecar
uses to reach the host's MCP surface. It is the **out-of-process dual of the wasm guest's
in-process `host.call-tool` bridge** — both are transports for the one MCP contract (rule 7),
denied identically by the host gate. It is **pure transport**: no grammar change, no WIT change.

## Goals

1. **A native sidecar can call any host/extension MCP tool it is granted**, out of process, over
   HTTP — `series.*`, `ingest.write`, `outbox.enqueue`, and other extensions' `<ext>.<tool>` —
   getting JSON back, exactly as the wasm guest's `host.call-tool` does in-process and the page
   bridge does from the browser.
2. **The child token is genuinely verifiable.** Replace the throwaway per-mint key with the node's
   one signing key, so the gateway *verifies* the child's `LB_EXT_TOKEN` on every callback — no
   throwaway, no co-trust hack, workspace + grant read from inside the token.
3. **One generic transport, reused by every native extension.** A single `lb-sidecar-client`
   crate — nothing extension-specific — that ros / mqtt / control-engine all reuse.
4. **Deny is first-class.** A capability/workspace 403 is a distinct, typed outcome
   (`CallError::Denied`), never a panic, never conflated with a transport error.

## Non-goals

- **No new tool surface.** The transport dispatches the *existing* verbs through the *existing*
  gateway `POST /mcp/call` → `call_tool` chokepoint. It adds no CRUD/list/watch verbs.
- **No grammar change, no WIT change.** This touches neither the capability grammar (four surfaces,
  unchanged) nor the SDK/WIT world. A native child is not a WIT guest; it speaks HTTP. This is
  transport only — flagged loudly per the non-negotiables.
- **No raw store/bus handle in the child.** A sidecar reaches the platform *only* through host MCP
  verbs over the transport (rule 5), never a direct query (rule 4: stateless).
- **No streaming/`watch` from a sidecar** this slice (request/response only). A sidecar subscribing
  to bus motion is a separate scope.
- **No change to the gateway's `/mcp/call` contract.** The transport is a new *client* of the
  existing endpoint; the endpoint is untouched.

## Intent / approach

**One signing identity on the `Node`, then one generic HTTP client.** Two pieces:

1. **Unify the signing identity.** The node's signing key moves onto `Node`
   (`key: Mutex<Arc<SigningKey>>`, with `Node::key()` / `Node::install_key()` — mirroring the
   existing `runtimes` / `install_runtimes` slot the role fills at build time, never a compile-time
   branch). `Gateway::build` installs *its* login/authenticate key onto the node, so the native
   tier's token minter (`native/spec.rs`, now taking `&SigningKey`) reads the **same** key the
   gateway verifies with. The child's `LB_EXT_TOKEN` is therefore a real node-signed JWT; the
   gateway verifies it on the callback, and the workspace + grant come from inside it
   (un-spoofable).

2. **The generic transport.** `lb-sidecar-client` reads the vars the supervisor injects
   (`LB_EXT_WS` / `LB_EXT_ID` / `LB_EXT_TOKEN` / `LB_GATEWAY_URL`), and
   `call_tool(tool, input)` POSTs `{tool, args}` to `{gateway}/mcp/call` with
   `Authorization: Bearer <token>`, mapping HTTP 403 → `CallError::Denied`. The gateway authorizes
   it through the same chokepoint the browser and the wasm guest hit — three transports, one
   contract, one gate.

**Alternative considered & rejected — keep the throwaway co-trust token, trust the loopback.**
Leave `spec.rs` generating its own key and have the gateway accept any well-formed token from
localhost. Rejected: the throwaway key is discarded the instant the token is minted, so it can
*never* be verified — "verify" would collapse to "don't verify," a hole rather than a transport.
And it would force a **fake verifier** (localhost-trusts-everyone) to make tests pass — the banned
"look done while the real path is unbuilt" (rule 9 / testing §0). The honest fix is structural and
cheap: give the node one key. Rejected the hack; took the key.

**Alternative considered & rejected — a bespoke socket / control-line proxy back to the host.**
The native-tier scope's default was "proxy the callback through the existing stdio control line
(one transport)." Rejected for the callback: the control line is a *host→child* command channel;
threading child→host→child MCP calls back up it re-implements HTTP routing over stdio and forks the
call path from the page/guest transports. The gateway already speaks `POST /mcp/call` for the
browser; a native child is just another HTTP client of it. One less transport to invent, one gate
they all share.

## How it fits the core

- **Tenancy / isolation:** the callback's workspace is the one **inside** the child's token (minted
  by the supervisor as `LB_EXT_WS`), verified by the gateway — never client-supplied. A ws-B child
  can only ever reach ws-B tools/data. Mandatory workspace-isolation test:
  `ws_b_callback_cannot_see_ws_a_series`.
- **Capabilities:** every callback runs the full `authorize_tool` gate at the gateway
  (workspace-first, then `mcp:<tool>:call`) against the child's scoped principal
  (`requested ∩ admin_approved`, the token the supervisor minted). Deny is opaque and typed
  (`CallError::Denied`). Mandatory deny test:
  `ungranted_sidecar_callback_is_denied_and_leaks_nothing`.
- **MCP surface:** **consumes** the existing tool surface over the existing `/mcp/call` endpoint;
  **exposes nothing new**. API-shape (§6.1): request/response only this slice; no CRUD/list/watch
  added.
- **Secrets:** the injected `LB_EXT_TOKEN` is the only secret material — now a real node-signed JWT
  (no longer a throwaway), carried in the `Authorization` header, never logged, never in a record.
- **Data (SurrealDB) / Bus (Zenoh):** none added. The sidecar touches the store only through host
  verbs via the transport (the one datastore, one mediated path); no new motion this slice.
- **Sync / authority:** unchanged; a routed `<ext>.<tool>` callback uses the existing cross-node
  MCP route beyond the gateway.
- **SDK/WIT impact:** **NONE.** No WIT world change, no grammar change. A native child is not a WIT
  guest; the transport is HTTP + a Rust client crate. This is the deliberate contrast with the
  host-callback ABI slice (which *was* a `@0.2.0` WIT bump for the wasm guest) — the native peer
  needs no forever-ABI change because it reuses the gateway's existing HTTP contract.

## Example flow

fleet-monitor's `fleet.probe` tool (the proof wiring): "find a series via a host callback."

1. The supervisor spawns fleet-monitor as a native child, minting `LB_EXT_TOKEN` with the **node
   key** carrying `granted = requested ∩ admin_approved` (which includes `mcp:series.find:call`),
   and injecting `LB_EXT_WS` / `LB_EXT_ID` / `LB_EXT_TOKEN` / `LB_GATEWAY_URL`.
2. A caller invokes `fleet.probe`. Inside it, the child calls
   `SidecarClient::from_env().call_tool("series.find", …)` → `POST {gateway}/mcp/call` with
   `Authorization: Bearer <LB_EXT_TOKEN>`.
3. The gateway **verifies** the token with the node key, authorizes `mcp:series.find:call` against
   the child's scoped principal in its workspace, discovers over the tag graph, and returns the
   matching series as JSON.
4. **Deny path:** mint a token *without* `mcp:series.find:call`; step 3 returns HTTP 403 → the
   client surfaces `CallError::Denied`, and no data leaks.

## Testing plan

Mandatory categories from `scope/testing/testing-scope.md`, all through a **real** node + **real**
axum gateway on a **real** TCP port, driven by the **real** `lb-sidecar-client` over **real**
`reqwest` HTTP (no mocks, no `oneshot` — `reqwest` needs a real socket; CLAUDE §9 / testing §0).
Series are made findable via **real tag edges** (`tags_add`), since `series.find` discovers over
the tag graph. `role/gateway/tests/native_callback_test.rs` (all 3 pass):

- **Happy round-trip** — `granted_sidecar_callback_reaches_series_find`: a granted child token
  round-trips over HTTP and returns a real seeded series.
- **Capability deny** — `ungranted_sidecar_callback_is_denied_and_leaks_nothing`: a token lacking
  `mcp:series.find:call` gets `CallError::Denied` (403); no data leaks.
- **Workspace isolation** — `ws_b_callback_cannot_see_ws_a_series`: a ws-B token sees none of ws-A's
  seeded series (workspace read from inside the token, un-spoofable).

## Risks & hard problems

- **The one-key change is load-bearing.** The gateway and the native minter must read the *same*
  key or the child token is un-verifiable (the exact bug this fixes). `install_key` fills the slot
  at role build time; a role that forgets to install it would mint tokens no one can verify — the
  tests boot the real gateway path to prove the wired case.
- **Deny must stay first-class.** A 403 mapped to a generic transport error would hide a real
  authorization failure as a network blip. `CallError::Denied` is a distinct variant, asserted by
  the deny test.
- **Un-spoofable workspace.** The workspace must come from *inside* the verified token, never from
  a client-supplied field — otherwise a compromised child could claim another tenant. The isolation
  test proves ws-B cannot reach ws-A.
- **AI-written / untrusted sidecars.** The whole safety story is "the child can do nothing the gate
  doesn't allow." The verified scoped token + gateway gate is what bounds an untrusted native child;
  the deny + isolation tests prove the path is real, not displayed.

## Related

- [`native-tier-scope.md`](native-tier-scope.md) — the slice this completes; its
  "child→host callback transport" open question is RESOLVED by this scope. Its throwaway
  `LB_EXT_TOKEN` co-trust hack is fixed here.
- [`host-callback-scope.md`](host-callback-scope.md) + `sessions/extensions/host-callback-session.md`
  — the **in-process wasm dual** (`host.call-tool` WIT bridge). This is its out-of-process peer over
  HTTP; same MCP contract, same gate, no WIT change.
- `scope/mcp/mcp-scope.md` — the authorize-then-dispatch gate the callback reuses at the gateway.
- `scope/agent/agent-scope.md` — the `requested ∩ admin_approved` scoped-token mint the supervisor
  reuses for the child's identity.
- `scope/auth-caps/auth-caps-scope.md` — the grammar this deliberately does **not** change.
- README `§6.5` (MCP as the contract), `§3` rules 4/5/7.
