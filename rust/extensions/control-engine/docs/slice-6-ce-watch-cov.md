# Slice 6 — `ce.watch` (live COV over the extension-watch primitive)

Status: scope slice (S6). Depends on: S4 (routing); sequenced against
`scope/extensions/extension-watch-scope.md` (see fallback). Parent:
`control-engine-scope.md`.

Surface CE's binary-WebSocket change-of-value stream as a workspace-scoped live feed:
`ce.watch` is a `kind = "watch"` streaming tool on the **generic** extension-watch
contract — the host allocates the subject, the owning node's sidecar arms its CE COV
subscription on first subscriber, frames ride Zenoh, the gateway relays SSE, and
`bridge.watch` feeds the wiresheet (S7). No CE knowledge enters core.

## The frame contract (the real design work in this slice)

The wiresheet's WS carries three message kinds, and the canvas needs all three —
`ce.watch` is not just values:

```jsonc
// every frame: { kind, ts, ... } — JSON on the bus subject, SSE data verbatim
{ "kind": "cov",      "ts": 171234, "values": [ { "uid": 1000100, "v": 4.2 }, ... ],
                                    "status": [ { "uid": 1000101, "s": 3 }, ... ] }
{ "kind": "topology", "ts": ...,    "msg": { /* TopologyMsg passthrough */ } }
{ "kind": "schema",   "ts": ...,    "msg": { /* SchemaMessage passthrough */ } }
```

- **`cov`** re-encodes `ce-client-rust`'s decoded frame (`cov.rs` `CovStream` already
  decodes the binary sections — the sidecar never re-implements `wire.ts`). Field set
  per the parent scope's open question: `uid + value` (+ `status` flags section when
  present) — NOT the whole binary layout. Types: JSON numbers; i64/u64 that exceed
  2^53 serialize as strings (decide once here; the wiresheet's `DecodedValue`
  already handles `bigint`).
- **`topology`/`schema`** pass CE's JSON WS messages through — the wiresheet resyncs
  structure via `ce.tree` on topology hints, it does not rebuild from the frame.
- Batching: the sidecar coalesces to the CE tick (CE already ticks server-side);
  no additional LB-side buffering in v1 — measure first (S8).

## Arm / disarm lifecycle

- `ce.watch { appliance, scope? }` → gate `mcp:control-engine.watch:call`
  (workspace-first) → resolve appliance (S4) → the extension-watch primitive
  allocates/reuses the subject for `(ws, control-engine, watch, args-hash)` and routes
  the **arm** to the owning node.
- On arm, the sidecar calls `subscribe_cov(scope)` on its local client (opening the
  binary WS lazily) and pumps decoded events → JSON frames → the allocated subject.
- Last subscriber gone → **disarm** → drop the `CovStream` (and the WS if it was the
  only consumer). `ce.appliance.remove` (S4) force-disarms any live watch for that
  appliance.
- Fire-and-forget motion; no persistence (rule 3). Reconnect/resubscribe on CE WS
  drop is the sidecar's job (bounded backoff — mirror `ws.ts`'s STABLE_MS idea).

## Sequencing fallback (decided in the parent scope)

If the extension-watch primitive hasn't landed when this slice starts: ship live COV
on the zero-core-change bridge — sidecar `ingest.write` onto a `flow:`-style series +
the shipped `series.watch` SSE — behind the **same** `ce.watch` tool name and frame
JSON, then swap the plumbing when the primitive lands. The frame contract above is
plumbing-agnostic on purpose; S7 must not care which one is underneath. Track the
migration as a named follow-up if the fallback ships first.

## Opt-in historian (small, separate deliverable)

A per-appliance list of props mirrored to the series plane (`ingest.write`), config on
the appliance record (`history: [prop-uid…]`), requesting `mcp:ingest.write:call` only
when used. Never all COV by default. Can land after the live path; keep it in this
slice's doc so it isn't re-scoped.

## Testing / exit gate

- Arm-on-first / disarm-on-last: subscribe twice, drop once → still armed; drop both
  → `ce_fake`'s COV subscription dropped (instrument it).
- **Routed watch:** two-node harness — watch from node A, appliance on node B; a COV
  event injected into B's fake arrives as an SSE frame on A's gateway. Workspace claim
  checked at arm; ws-B caller cannot watch ws-A's appliance (isolation).
- Deny: no `mcp:control-engine.watch:call` → denied before any arm.
- CE WS drop mid-watch → sidecar reconnects and resumes; subscriber sees a gap, not a
  dead stream (assert via the fake's reconnect counter).
- Real-engine (opt-in): patch a prop via `ce.patch`, observe the COV frame arrive on
  the SSE stream.
- **Exit gate:** routed watch green end to end (subscribe → arm → frame → SSE) +
  the lifecycle/deny/isolation matrix green.
