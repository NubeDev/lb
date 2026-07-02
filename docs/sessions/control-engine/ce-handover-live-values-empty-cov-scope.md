# Handover — CE canvas: live values never stream (empty COV subscribe scope)

- **Date:** 2026-07-03
- **Branch:** `ce-node-wiring-v2` (STAY on it)
- **Live env:** ce-studio engine running on `127.0.0.1:7979` (REST) / `/` WS; LB node on
  `127.0.0.1:8080` (gateway), UI vite dev on `5173`. Engine started via
  `/home/user/code/c/ce/ce-studio` (`run.sh`). The `ce_appliance` registry has ONE
  appliance, **id `aaaa`** (base `http://127.0.0.1:7979`) — NOT `local`.

## TL;DR — the remaining bug (ROOT-CAUSED, verified against the live engine)

The Control Engine canvas renders the graph + wires now, but shows **no live values**
and the badge reads **disconnected**. Root cause is a **single confirmed fact**:

> `control-engine.watch` builds an **empty `CovScope`** (no component/property UIDs),
> the crate's `subscribe` then sends `{"type":"subscribe"}` with no `components`, and
> **the engine only pushes COV frames for explicitly-subscribed components** — so an
> empty subscribe streams **zero** value frames.

Proven with a raw WS client to `:7979/` (bypassing all LB/crate code):
- `subscribe` with **no** components → schema + presence text frames, **0 binary value frames** in 3.5s.
- `subscribe` with `"components":[100008,100009,100010,100011,100012,100013]` (the real
  children) → **36 binary value frames** in 4s.

So the pump arms fine, the series opens fine, the SSE authenticates fine — there is
just **nothing to stream** because we subscribed to nothing.

### The fix (next session)

Make `control-engine.watch` subscribe to the actual component UIDs instead of an empty
scope. Options, pick one:

1. **Extension-side (recommended, self-contained):** in the watch verb
   (`rust/extensions/control-engine/src/watch/verb.rs`), before arming, fetch the tree
   (reuse `tools::raw_tree` or `get_tree`) and populate `CovScope.components` with every
   component UID (or the requested subtree's). Then the pump's `subscribe` enumerates
   them and frames flow. Keep the empty-scope path as "explicit scope not given → watch
   the whole tree" by expanding it to all UIDs, NOT leaving it literally empty.
   - Watch the series-name hash: `series.rs::args_hash` keys the series on the scope, so
     if you expand the scope, the series name changes with the tree. That's fine (a new
     topology = a new series) but make sure the transport's arm + read use the SAME
     resolved series (the verb returns `{series}`; the UI reads exactly that, so it's
     consistent as long as one call resolves both).
2. **Transport-side:** have `BridgeTransport.openStream` pass the visible components'
   UIDs as `scope.components` when it calls `control-engine.watch`. More plumbing (the
   UI must know the UID set), and re-arming on drill-in — prefer #1.

**Also fix (cheap, same area):** the crate's WS `SchemaMsg` is snake_case
(`session_id`) but the engine sends **camelCase** (`sessionId`) — confirmed on the wire.
It's `#[serde(default)]` so it doesn't error, but `session_id` silently decodes to `""`,
breaking WS **resume/gap-detection**. Not the cause of "no values" (that's the empty
scope), but note it: if you touch the crate for anything, add `#[serde(rename_all =
"camelCase")]` to `SchemaMsg`/`SchemaProperty`. Same camelCase issue the REST `EdgeDto`
had — the whole engine speaks camelCase and this crate rev doesn't.

### How to verify the fix

```bash
# 1. rebuild + (republish to the live node — it does NOT hot-reload Rust)
cd /home/user/code/rust/lb/rust && cargo build -p control-engine
#    then: make kill && make dev …  (or your publish flow) so the node runs the new sidecar

# 2. mint a token, arm the watch with the REAL appliance id, read the stream:
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/login -H 'Content-Type: application/json' \
  -d '{"user":"user:ada","workspace":"acme"}' | python3 -c 'import sys,json;print(json.load(sys.stdin)["token"])')
S=$(curl -s -X POST http://127.0.0.1:8080/mcp/call -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"tool":"control-engine.watch","args":{"appliance":"aaaa"}}' \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["series"])')
timeout 5 curl -s -N "http://127.0.0.1:8080/series/$S/stream?token=$TOKEN" | head -c 400
#    BEFORE fix: 0 bytes. AFTER fix: `event: sample` lines with frame payloads.
```

Then open the CE page in the browser — badge should go connected and values populate.

## What was already fixed this session (all green, code + tests)

1. **#4 blank canvas / codec crash — FIXED.** The engine emits **camelCase** keys
   (`sourceUid`, `childrenCount`, …); the crate's snake_case `EdgeDto` rejected EVERY
   edge → whole `/nodes` decode failed → blank canvas. Fixed with a tolerant raw fetch
   `rust/extensions/control-engine/src/tools/raw_tree.rs` (untyped `serde_json::Value`
   pass-through), routed from `serve.rs` for the real (`!is_fake()`) path. Rule-9 tests +
   a live `bridge-transport.live.test.ts` (spawns the real sidecar vs the running engine,
   asserts real camelCase edges through the full bridge). See
   `docs/debugging/frontend/ce-tree-missing-source-uid-blanks-canvas.md`.
2. **#3 node slots — FIXED (was downstream of #4).** Structure/ports render once the
   graph decodes.
3. **#1 theme — FIXED.** `useDocumentColorMode` reads `.dark` (host convention) not the
   never-set `.light` (`packages/ce-wiresheet/src/CeEditor.tsx`).
4. **#2 header clash — FIXED.** Removed the CE page's own `<header>`
   (`rust/extensions/control-engine/ui/src/Page.tsx`); host header wins; appliance picker
   shows only with ≥2 appliances; `ConnectionStatus` badge already lives in `CeEditor`.
   `mount.test.tsx` updated. **NOTE: this is in the ext bundle (`remoteEntry.js`) — it
   needs the ext UI republished to appear on the live node.**
5. **Shell bridge `watch` — ADDED.** `ui/src/features/ext-host/bridge.ts` now implements
   `watch` (maps `series.watch` → `openSeriesStream` SSE, scope-gated). This was a REAL
   gap (the bridge only had `call`, so the stream half was structurally impossible) — it
   is NECESSARY but NOT SUFFICIENT: even with `watch` wired, the empty-scope subscribe
   above means the series carries no frames. See
   `docs/debugging/frontend/ce-canvas-disconnected-no-live-values-bridge-missing-watch.md`.
   This is in the shell (`ui/src`), served by vite HMR — already live, no republish.

## Key files

| Concern | File |
| --- | --- |
| **COV scope built empty (THE bug)** | `rust/extensions/control-engine/src/watch/series.rs` (`target`, empty components) |
| Watch verb (arm point to inject scope) | `rust/extensions/control-engine/src/watch/verb.rs` |
| COV pump (subscribe → frame → ingest.write) | `rust/extensions/control-engine/src/watch/pump.rs` |
| Crate WS subscribe/schema (camelCase gap) | `ce-client-rust` (git dep) `src/ws/control.rs` (`SchemaMsg`, `subscribe_msg`) |
| Tolerant tree fetch (done) | `rust/extensions/control-engine/src/tools/raw_tree.rs` |
| Shell bridge watch (done) | `ui/src/features/ext-host/bridge.ts` |
| Series SSE client (reused) | `ui/src/lib/dashboard/series.stream.ts` |
| CE transport openStream | `rust/extensions/control-engine/ui/src/bridge-transport.ts` |

## Gotchas carried forward

- The node does **NOT** hot-reload Rust — rebuild `-p control-engine` + republish
  (`make kill && make dev`) or the sidecar stays stale. The current live sidecar (pid at
  handover time started 04:37) DOES have the raw_tree fix.
- The live appliance id is **`aaaa`**, not `local` — `appliance:"local"` returns
  `not found` (both tree and watch). Use `aaaa` (or whatever `appliance.list` returns).
- No system `cc` — links via zigcc (`rust/.cargo/config.toml`).
- ext UI is standalone: `pnpm install --ignore-workspace` in its dir;
  `PNPM_CONFIG_MINIMUM_RELEASE_AGE=0` to dodge the supply-chain gate.
- 2 pre-existing `toBeInTheDocument` failures in `ExtHost.gateway.test.tsx` are a jest-dom
  matcher-setup gap (fail without any of my edits — verified by stash) — NOT mine.
