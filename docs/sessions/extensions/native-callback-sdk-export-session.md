# Session — publish the native host-callback client through the SDK

- **Scope:** [`scope/extensions/native-callback-sdk-export-scope.md`](../../scope/extensions/native-callback-sdk-export-scope.md)
- **Builds on:** [`scope/extensions/native-callback-transport-scope.md`](../../scope/extensions/native-callback-transport-scope.md)
  (the transport, shipped 2026-07-02) and [`ext-out-of-tree-scope.md`](../../scope/extensions/ext-out-of-tree-scope.md)
  (the SDK-owns-the-contract cutover).
- **Status:** shipped. SDK green (17 + 2 + 2 doctests); lb end-to-end green (3/3 real-gateway callback tests).
- **Date:** 2026-07-11.
- **Tags:** `NubeDev/lb-ext-sdk` → **`sdk-v0.3.0`**; `NubeDev/lb` → **`node-v0.3.0`**.

## The gap, restated

The native-callback transport (`SidecarClient` — a native sidecar's out-of-process peer of the wasm
guest's `host.call-tool` bridge) shipped in 2026-07 as an **in-tree `lb` crate**
(`rust/crates/sidecar-client`), consumed by the in-tree extensions (`ros`, `fleet-monitor`,
`control-engine`) via a workspace **path** dependency.

After the out-of-tree cutover, an out-of-tree native extension pins the platform contract by **git
tag** from `NubeDev/lb-ext-sdk` — `lb-ext-native` (the child wire) and `lb-sdk` (the WIT world). The
callback client was **never published there**. So a native extension had the host→child control wire
but **no way to call back** into the host's MCP surface. A downstream product hit this precisely: its
native authz chokepoint needed to call the already-shipped generic verbs `authz.check_scoped` /
`authz.scope_filter` from a native-tier extension, and the SDK exposed no host-callback client. The
wasm (WIT) tier could reverse-call; the native tier could not — a **distribution** gap, not a design
gap. (Tellingly, `lb-ext-native`'s own `Cargo.toml` + `README` already *claimed* it provided "the
host-callback client" — aspirational, and now true.)

## The decision — a separate SDK crate, re-exported from `lb-ext-native`

Two live options (asked; user chose "whatever's best long-term, no quick fixes"):

1. **Publish `lb-sidecar-client` as its own crate in the SDK, and re-export it from `lb-ext-native`.** ← chosen
2. Merge the client into `lb-ext-native`.

Chose (1): the HTTP callback transport is a **distinct responsibility** from the stdio control wire
(FILE-LAYOUT — one responsibility per crate), and it mirrors lb's own split (`lb-supervisor` = wire,
`lb-sidecar-client` = callback) and the WIT tier's shape (`lb-sdk` owns the guest's reverse-call
surface separately). Re-exporting from `lb-ext-native` keeps the consumer ergonomics of (2) — a native
extension still carries **one** platform dependency for both directions of the wire — without
collapsing two responsibilities into one crate. It also makes lb's monorepo consume the client **back
from the SDK** by git tag (dropping the in-tree path crate), so there is one source of truth, exactly
the posture lb already holds for `lb-sdk`.

## What shipped

**In `NubeDev/lb-ext-sdk` (`sdk-v0.3.0`, PR #1 merged):**

- New crate `crates/lb-sidecar-client` — `SidecarClient` / `Config` / `CallError`, byte-for-byte the
  shipped transport (only its crate home + doc comments moved; product-specific example text kept out
  of the public repo).
- `lb-ext-native` re-exports `SidecarClient`, `Config`, `CallError` (and the `lb_sidecar_client`
  module) — one dependency, both directions of the wire.
- Workspace `Cargo.toml`: new member + `thiserror`/`reqwest` workspace deps; version → `0.3.0`.
- README: new crate row + a native-extension callback snippet; tag refs bumped to `sdk-v0.3.0`.
- **Bug fix** (latent, pre-existing): `lb-ext-native`'s `serve` test harness dropped only the write
  half of the host `duplex`, so a non-shutdown test hung on the never-arriving EOF. Drop both halves.
  See [`debugging/extensions/native-serve-test-eof-hang.md`](../../debugging/extensions/native-serve-test-eof-hang.md).

**In `NubeDev/lb` (`node-v0.3.0`):**

- `rust/Cargo.toml`: `lb-sidecar-client` path dep → `{ git = ".../lb-ext-sdk", tag = "sdk-v0.3.0" }`;
  removed `crates/sidecar-client` from workspace members.
- Deleted the in-tree `rust/crates/sidecar-client/` crate.
- `extensions/control-engine/Cargo.toml` (a workspace-**excluded** native ext, so it can't inherit
  `{ workspace = true }`): direct git dep on the same tag.
- (`fleet-monitor`/`ros` inherit via `{ workspace = true }` — unchanged lines, new source.)

The host end of the wire (the gateway `POST /mcp/call` endpoint + the capability gate) is **unchanged**.
No WIT change, no `PROTOCOL_MAJOR`/`WORLD_MAJOR` bump, no new verb, no grammar change.

## Tests (real infra, no mocks)

**SDK repo** — `crates/lb-ext-native/tests/host_callback_test.rs`: the re-exported `SidecarClient`
drives a real `reqwest` call against a **real** `tokio` TCP server speaking the `/mcp/call` shape
(not a mock of the client — a genuine HTTP peer). Asserts the wire contract (`POST /mcp/call`,
`Authorization: Bearer`, `{tool,args}`, **no `ws` in the body**), the round-trip decode, and the
mandatory deny (`403 → CallError::Denied`).

```
test result: ok. 17 passed   # lb-ext-native lib (incl. the un-hung serve tests)
test result: ok.  2 passed   # host_callback_test: round-trip + deny
Doc-tests lb_ext_native / lb_sidecar_client: ok
```

**lb repo** — `role/gateway/tests/native_callback_test.rs` (unchanged source, now consuming the
git-tagged crate): the three real-gateway tests are the mandatory categories.

```
running 3 tests
test result: ok. 3 passed; 0 failed
# granted_sidecar_callback_reaches_series_find (round-trip)
# ungranted_sidecar_callback_is_denied_and_leaks_nothing (capability-deny)
# ws_b_callback_cannot_see_ws_a_series (workspace-isolation)
```

Consumability verified the way downstream pins it: `cargo build` of `lb-role-gateway` (workspace),
`fleet-monitor`/`ros-sidecar` (workspace), and `control-engine` (excluded ext) all fetched
`sdk-v0.3.0` from GitHub and built green — a **git tag**, not a local path.

## Caps / grants (what a calling extension must hold)

Unchanged from the transport scope: the extension's manifest must **request** `mcp:<verb>:call` for
each host verb it calls back (e.g. `mcp:authz.check_scoped:call`, `mcp:authz.scope_filter:call`), and
an admin must **approve** it — `granted = requested ∩ admin_approved`. The child's injected
`LB_EXT_TOKEN` carries that grant; the gateway authorizes each callback against it. An ungranted verb
→ `CallError::Denied`.

## Self-check

- [x] Scope satisfied; the SDK exposes the native host-callback client, re-exported for one-dep consumers.
- [x] No core branch on any extension id; the client is verb-agnostic (rule 10).
- [x] No mocks — real HTTP server (SDK) + real gateway (lb).
- [x] Mandatory deny + workspace-isolation tests present and green (lb e2e).
- [x] Bug fixed with a regression check + debug entry.
- [x] Tags are real fetchable git tags (verified by building against them).
- [x] STATUS + public promotion updated.
