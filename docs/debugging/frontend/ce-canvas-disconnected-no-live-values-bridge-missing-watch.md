# CE canvas shows "disconnected" and no live values — shell bridge had no `watch`

- **Date:** 2026-07-03
- **Area:** frontend / ext-host bridge
- **Status:** resolved
- **Branch:** `ce-node-wiring-v2`

## Symptom

After the tree decode was fixed (the graph and wires render — see
[ce-tree-missing-source-uid-blanks-canvas.md](ce-tree-missing-source-uid-blanks-canvas.md)),
the Control Engine canvas still showed **no property values**, and the
`ConnectionStatus` badge read **disconnected**.

## Root cause

Live COV values arrive over a stream, not the REST tree read. The CE page's
`BridgeTransport.openStream` (ext UI) arms the feed via
`bridge.call('control-engine.watch')` and then reads the returned series through
**`bridge.watch('series.watch', { series }, onEvent)`** (the shipped
`GET /series/{series}/stream` SSE). If the bridge has no `watch`, `openStream`
reports `onStatus("closed")` and degrades to a static canvas — by contract:

```ts
// bridge-transport.ts
if (typeof this.bridge.watch !== "function") { handlers.onStatus("closed"); return staticStream; }
```

The CE contract (`contract.ts`) declares `watch?` as **optional**, and the vendored
transport degrades gracefully when it's absent. But the shell's own
`ExtBridge`/`makeBridge` (`ui/src/features/ext-host/bridge.ts`) only ever
implemented **`call`** — it never provided `watch`. So for EVERY federated page the
`watch` seam was missing, `openStream` always took the "no transport" branch, and the
badge sat at disconnected with zero live values. The extension manifest was fully
correct (`series.watch` + `control-engine.watch` in the `[ui]` scope); the WS to the
engine was fine (root `/` on `:7979` upgrades). The gap was purely the un-wired
shell-side `watch`.

## Fix

Implement `watch` in `makeBridge` (`ui/src/features/ext-host/bridge.ts`), mapping
`series.watch` onto the existing `openSeriesStream` SSE client
(`lib/dashboard/series.stream.ts`) that the dashboard already uses:

- scope-gate it exactly like `call` (`series.watch` must be in the page's granted
  scope) — defense in depth; the gateway's SSE route re-authenticates the session
  token regardless;
- guard a non-`series.watch` verb / empty-or-missing `series` arg → no-op unsubscribe;
- return `() => stream?.close()`; when there is no gateway/EventSource (Tauri, tests)
  `openSeriesStream` returns null and the page degrades to a static canvas, unchanged.

Each SSE `Sample` IS the `{ payload, seq, ts, … }` event `BridgeTransport` pulls its
frame from, so no reshaping is needed.

## Regression test

`ExtHost.gateway.test.tsx` — `watch() gates series.watch on scope and requires a
series arg (no-op otherwise)`: asserts an ungranted page, a non-series verb, and an
empty/missing series all yield a clean no-op unsubscribe, and a granted+valid call
returns a callable unsubscribe (the live SSE itself is covered by the Rust
series-stream tests). Pure local logic, matching the sibling `call` out-of-scope test.

## Lesson

An OPTIONAL capability in a contract silently no-ops when the host forgets to provide
it — the page "works" (structure loads) while the live half is dead, with only a small
badge to hint at it. When a contract says `watch?`, the shell that owns the seam must
actually wire it (or the whole feature class — every federated page's live feed — is
quietly off). The manifest scope being correct masked it: the tool was granted, just
never streamable.
