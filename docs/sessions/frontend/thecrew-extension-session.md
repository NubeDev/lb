# Session — thecrew: the lift, playground → LB extension (graphics-canvas phases 1–2)

- Topic: frontend / graphics-canvas
- Status: done (2026-07-02)
- Scope (the ask): `rust/extensions/thecrew/docs/thecrew-extension-scope.md`
  (parent, authoritative for schema/engine/phases 3–5:
  `docs/scope/frontend/graphics-canvas-scope.md`)
- Public: `docs/public/frontend/graphics-canvas.md`
- Debug: `docs/debugging/frontend/thecrew-standalone-pnpm-install-walks-to-repo-workspace.md`
- STATUS row: "thecrew → the graphics-canvas extension (phases 1–2)" — moved scoped → **tested**.

## The ask, restated (my acceptance criterion)

Turn the proven playground into a **publishable, installable LB extension** with **zero core
additions**: a federated `[ui]` graphics page + a read-only `[[widget]]` scene cell on one remote;
scenes persisted as workspace docs through the shipped `assets.*` verbs; live values fed through the
host-mediated bridge (viewer's grant) instead of the simulator. `scene/canvas/theme/editor/state/`
lift unchanged; the new code is the packaging (wasm stub + build), the mount shell, the bridge value
source, and scene I/O. Hold the mandatory test categories (capability-deny, workspace-isolation) and
re-green the lifted vitest suite minus the deleted simulator tests.

## What shipped

**Packaging (Rust, proof-panel pattern):**
- `rust/extensions/thecrew/Cargo.toml` — own crate, `crate-type=["cdylib"]`, wasm target; added
  `extensions/thecrew` to the host workspace `exclude` in `rust/Cargo.toml`.
- `src/lib.rs` — a Tier-1 wasm32-wasip2 component that serves **zero tools** (proof-panel minus the
  tool handlers): it implements the `@0.2.0` world's `tool.call` and answers every name with an
  explicit error (never a silent success). Exists only because the loader + registry publish path
  require component bytes; there is no UI-only tier and adding one would be core surface (rejected).
- `extension.toml` — the manifest from the scope sketch: `tier=wasm`, six requested caps, the `[ui]`
  page scope (read+write) and the narrower `[[widget]]` scope (read-only, no `put_doc`/`list_docs`).
- `build.sh` — wasm component + federated UI bundle. Installs the UI with `--ignore-workspace`
  (see debug entry).

**UI lift (`ui/`, the federation remote):**
- `vite.config.ts` — a lib build emitting one ESM `dist/remoteEntry.js`; React (+ entry points)
  externalised to the host import map, three.js bundled (the federation payoff — only this remote
  carries the engine). CSS injected `?inline`. `pnpm dev` still runs the standalone playground.
- `src/remoteEntry.ts` — injects compiled CSS once, re-exports `mountPage`/`mountWidget`.
- `src/mount.tsx` — `mountPage(el, ctx, bridge)` and `mountWidget(el, ctx, bridge, widgetId)`; the
  widget reads its `sceneId` from `ctx.options`/`ctx.binding`.
- `src/bridge/contract.ts` — the frozen `Bridge` (call-only page) + `WidgetBridge` (call + optional
  `watch`) types, mirroring the shell.
- `src/bridge/ScenePage.tsx` — the page shell: the lifted `<App/>` (unchanged) wrapped with a
  persistence bar (scene picker + title + save + honest status) and the bridge `ValueSourceContext`.
- `src/bridge/SceneWidget.tsx` — the read-only cell: `<SceneCanvas/>` over the bridge source, no
  editor chrome, no save; honest loading/empty states.
- `src/bridge/scene-io.ts` — load/save/list scenes via `assets.get_doc`/`put_doc`/`list_docs`;
  the `scene:` id-prefix + `scene` tag convention; the last-writer-wins interim (read-before-write
  content compare → `SceneConflictError` "changed underneath you").
- `src/bridge/bridge-source.ts` — the ValueSource multiplexer: collect+dedupe the doc's bound
  channels, ONE upstream per series (backfill `series.latest`, then `series.watch` when the bridge
  offers `watch`, else poll `series.latest` at 2 s), fan out; a denied series → `null` (no-access).
- `src/data/empty-source.ts` — the inert default ValueSource (replaces the deleted simulator).
- `src/state/scene-store.ts` — **one additive method** `loadDoc(doc)` (normalises + clears history)
  so the mount can inject a fetched scene; the playground store had no arbitrary-scene loader. This
  is the one deviation from "state/ lifts unchanged" — additive, UI-layer, noted here.

**Deleted (rule 9):** `src/data/simulator.ts` + `src/data/simulator.test.ts`. Demos/tests now get
values through the ValueSource seam (real seeded series in the gateway suite; stub in unit tests).

## Decisions & why

- **`loadDoc` added to the store.** The scope says `state/` lifts unchanged, but the playground store
  self-inits to the AHU demo and exposes no way to load an external doc. Loading a persisted scene is
  the whole point of the lift. Chose the smallest additive change (a new method, existing behaviour
  untouched) over reworking `App`/`Toolbar` or threading a scene prop through the unchanged tree.
- **Scene discovery = `scene:` id-prefix, not tag filter.** `assets.list_docs` returns only
  `{id,title}` (no tags — verified in `crates/host/src/assets/tool.rs`), so a tag-side picker filter
  is impossible without a core change. Resolves parent-scope Open question 3 to the prefix; docs are
  still tagged `scene` for a future tag-returning list.
- **Last-writer-wins interim, not a `put_doc` fork.** `put_doc` has no revision check; the interim is
  a client read-before-write compare against the loaded baseline, surfacing a conflict honestly. The
  real fix stays a generic `document-store/` revision ask.
- **`watch` optional on the bridge.** The frozen page bridge is call-only; only the widget tier may
  carry `watch`. The source polls `series.latest` when `watch` is absent (parent OQ2: polling is fine
  for phases 1–2). No new transport requested.

## Findings surfaced (not built — zero core additions held)

1. `assets.put_doc` is still last-writer-wins (no `expected_rev`). Interim shipped; generic ask stands.
2. `assets.list_docs` returns no tags → prefix-based discovery (above).
3. The member/dev cap set lacks `mcp:assets.put_doc:call` (it has `store:doc/*:write` + `mcp:*.write`
   wildcards, but not the exact verb cap `put_doc` needs at MCP gate 1). A real scene save works only
   because a thecrew install grants `mcp:assets.put_doc:call`; the positive gateway tests mint that
   grant explicitly (`signInWithCaps`). The deny test proves a viewer without it is refused server-side.
4. `series.watch` SSE has no gateway-vitest transport (matches proof-panel): backfill is proven live,
   `watch` fan-out is proven via the widget stub; a Playwright e2e is the honest place for live SSE.

None required a new verb/table/cap/WIT surface — I stopped and recorded each instead of building it.

## Tests + green output

Unit (`cd rust/extensions/thecrew/ui && pnpm test`) — 50/50, incl. the lifted 31 (simulator suite
removed) re-pointed at the ValueSource seam, plus the new seams:

```
 ✓ src/editor/use-snap.test.ts (6 tests)
 ✓ src/scene/defaults.test.ts (4 tests)
 ✓ src/bridge/scene-io.test.ts (8 tests)
 ✓ src/scene/validate.test.ts (7 tests)
 ✓ src/bridge/bridge-source.test.ts (7 tests)
 ✓ src/state/scene-store.test.ts (9 tests)
 ✓ src/canvas/scene-render.test.tsx (5 tests)
 ✓ src/mount.test.tsx (4 tests)
 Test Files  8 passed (8)
      Tests  50 passed (50)
```
`pnpm typecheck` — clean.

Gateway (`cd ui && pnpm test:gateway src/features/ext-host/TheCrew.gateway.test.tsx`) — real spawned
gateway, real seeded series + docs (no fakes):
```
 ✓ src/features/ext-host/TheCrew.gateway.test.tsx (6 tests)
    ✓ seed→load→edit→save→reload round-trip is byte-stable through assets.put_doc/get_doc
    ✓ binding backfill: series.latest delivers a seeded sample to a bound channel
    ✓ capability deny: a viewer without the put_doc grant is DENIED a save (real host gate)
    ✓ capability deny (client scope filter): the widget grant cannot reach list_docs/put_doc
    ✓ workspace isolation: a scene saved in ws A is invisible from ws B
    ✓ widget no-access: a viewer denied the bound series gets an honest empty backfill
 Test Files  1 passed (1)
      Tests  6 passed (6)
```

Rust: `cargo build --workspace` ✓, `cargo fmt --check` ✓, `cargo test` (thecrew ext) 1/1 ✓.
Federation: `./build.sh` emits `ui/dist/remoteEntry.js` (1.9 MB; React external, three.js bundled,
both `mountPage`+`mountWidget` present, CSS inlined). Hot-reload: both mounts return an unmount that
empties the element (stateless), asserted in `mount.test.tsx`.

## Not built (parent-scope phases 3–5)

AI drawing + `skills/graphics-canvas/SKILL.md`, symbol packs, 3D-first. Shape `action` wiring
(click-to-command) and multi-user co-editing remain non-goals here.
