# Session — thecrew: the lift, playground → LB extension (graphics-canvas phases 1–2)

- Topic: frontend / graphics-canvas
- Status: **shipped** — published + installed + driven live in a browser (Session 2, 2026-07-02)
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

---

# Session 2 — publish + install + drive live (tested → shipped)

Session 1 left thecrew **tested** but never PUBLISHED into a running node or driven in a real browser.
This session finishes that: publish the signed artifact, install it with the requested grant, seed the
AHU-1 demo through the real `assets.*`/`ingest` verbs, and verify LIVE in a built shell — the page AND
the read-only dashboard widget — with green Playwright e2e + screenshots.

## What shipped (live path)

- **Publish/install is the generic `make publish-ext EXT=thecrew` path** (pack the signed artifact →
  `POST /extensions` → verified+installed+loaded → deploy `ui/dist` to `extensions-ui/thecrew/`).
  Confirmed live: HTTP 204, `GET /extensions` shows the `Graphics` `[ui]` slot + the `Scene`
  `[[widget]]` tile, and `/extensions/thecrew/ui/remoteEntry.js` serves 200 at the exact path the
  manifest `entry` resolves to (`remoteEntry.js`, no `assets/` prefix — self-consistent).
- **Finding 3 resolved (the real save/load blocker):** the page bridge (`POST /mcp/call`) authorizes
  against the **logged-in user's** caps, NOT the install grant. The dev-login `member_caps()` carried
  `store:doc/*:write`/`mcp:*.write` wildcards but **none of the exact `assets.*` verb caps** nor
  `series.watch` — so a live scene load/save/live-feed 403'd. Added the existing caps
  `mcp:assets.{get_doc,put_doc,list_docs}:call` + `mcp:series.watch:call` to `member_caps`
  (`rust/role/gateway/src/session/credentials.rs`) — **existing verbs, a grant-config change, zero
  core additions** (proof-panel's precedent: "dev claims +mcp:…"). Gateway crate tests 13/0 after;
  the TheCrew deny tests still deny (they mint a narrower `/_seed/session` token, not `member_caps`).
- **`make seed-thecrew` + `seed-demo.sh`** (parent scope Open question 4, "first-run create demo
  scenes"): seed the AHU-1 scene doc (`assets.put_doc`, id `scene:ahu-1`, content_type json, tag
  `scene`) + its 8 bound `ahu1.*` series (`ingest`) + a read-only "Graphics Scene" dashboard
  (`dashboard.save`). All through the REAL host verbs (no fakes); idempotent; `docs/ahu-1.scene.json`
  is the canonical demo scene (mirrors `scene/demo/ahu-demo.ts`).

## Two live-only bugs (green in unit+gateway, broke in the real browser) — found + fixed

1. **`process is not defined`** on remote load — three.js/@react-three/fiber (bundled here) read
   `process.env.NODE_ENV`, which a Vite **lib** build doesn't inject. Fixed with `define` in
   `vite.config.ts`. [debug](../../debugging/extensions/federated-lib-build-leaks-process-env-node-env.md)
2. **`remote does not export a \`mount\` function`** — the shell's `pickMount` wants the page export
   named **`mount`** (frozen contract), but thecrew exported `mountPage`. Fixed: export `mount` +
   `mountPage` alias. [debug](../../debugging/extensions/federated-page-export-must-be-named-mount.md)

Both were invisible to Session 1's 50 unit + 6 gateway tests (Node/jsdom has `process`; the unit
`mount.test.tsx` imports `mountPage` directly, bypassing `pickMount`). The honest guard is the
live-shell e2e — added this session.

## Live verification (real Chromium, built shell :4173, real node :8080)

- `ui/e2e/thecrew.spec.ts` (**1/1 green**): login → **Graphics** nav slot → federated page mounts
  in-process (no `process`/hook/module-specifier errors) → scene picker lists **AHU-1** (via
  `assets.list_docs`, `scene:` prefix) → select → title populates → **SF-1's live speed `1800`**
  reaches the PropertyRail (the same value that spins the impeller in `useFrame`) via `series.latest`
  over the bridge → **drag** SF-1 (store `nudge`) → **Save** (`assets.put_doc`) → status "saved" →
  **reload** the scene clean. Screenshot: `docs/shots/graphics-ahu-1-live.png` (full airflow train
  rendered with live values: ΔP 0.4, SAT 14.1, coil 22.4°/14.1°, SF-1 running ring).
- `ui/e2e/thecrew-widget.spec.ts` (**1/1 green**): open the seeded "Graphics Scene" dashboard → the
  **`ext:thecrew/scene` cell mounts in-process**, renders the scene `<canvas>` (NOT the empty state),
  and carries **NO persistence bar / Save button** (read-only widget tier). Screenshot:
  `docs/shots/scene-widget-dashboard.png`.
- Test seams (UI-layer, not fakes): `window.__tcStore` exposed in `mountPage` (deterministic
  select/nudge — WebGL pointer-picking is unreliable headless; real save still flows through the
  bridge); `mountWidget` now also reads `sceneId` from `ctx.vars` (the dashboard's `ExtWidget` passes
  `vars`; `options`/`binding` are `{}` — see finding below). Contract `WidgetCtx` fields made optional.

## New findings surfaced (Session 2) — STOP-and-surfaced, not built

5. **A Vite lib federated remote must define `process.env.NODE_ENV` itself** (bug 1 above). A shared
   federation-remote vite preset carrying it would stop every new bundling extension rediscovering it.
6. **The federation PAGE export must be named `mount`** (bug 2). Consider a unit assertion that the
   remoteEntry has a `mount` export, to fail fast without a browser.
7. **The dashboard PanelEditor source picker dropped the "Extension widgets" group.** A concurrent viz
   panel-editor rework superseded `WidgetBuilder.tsx` (which had the `widget` group) with
   `editor/PanelEditor` → `QueryTab`, whose picker renders only `series`/`live`/`sql`/`extension` — so
   a packaged `[[widget]]` tile **cannot be added through the live builder UI today**. This is core
   dashboard-editor surface (not thecrew), so per "zero core additions, STOP and surface" I seeded the
   `ext:thecrew/scene` cell directly via `dashboard.save` to exercise the identical render path.
   Follow-up: restore the `widget` `PickerGroup` in `QueryTab` (`extWidgetEntries` still exists).
8. **`ExtWidget.tsx` passes `options:{}`/`binding:{}` — a cell's `options.sceneId` never reaches the
   widget.** The scope's intended cell shape is `{view, options:{sceneId}}`, but the renderer forwards
   only `ctx.vars`. Interim: thecrew reads `sceneId` from `ctx.vars` (a `const` dashboard var) too; the
   generic fix is `ExtWidget` forwarding `cell.options` into the widget ctx (a dashboard-core change).
9. **SurrealKV `Invalid revision` on `assets.list_docs` after repeated writes** — the same pre-existing
   persistent-engine bug proof-panel hit ([store/surrealkv-invalid-revision-on-drain-reread.md](../../debugging/store/surrealkv-invalid-revision-on-drain-reread.md)).
   Repeated ingest/save cycles corrupted the on-disk store; the scan verb (`list_docs`) threw while a
   keyed `get_doc` still worked. Live demo runs on the **in-memory** engine (no `LB_STORE_PATH`),
   matching proof-panel's mitigation. Not a thecrew bug.

The widget cell's canvas renders visually blank in a small grid cell (camera/viewport fit) though the
doc loads and the tier/read-only contract hold — the parent scope's "WebGL contexts/fit per dashboard
cell" risk; noted, functional contract proven.

## Not built (parent-scope phases 3–5)

AI drawing + `skills/graphics-canvas/SKILL.md`, symbol packs, 3D-first. Shape `action` wiring
(click-to-command) and multi-user co-editing remain non-goals here.

---

# Session 3 — close the surfaced findings (5–9) + the blank-cell fit (tested → shipped)

Session 2 shipped the live path but left the SURFACED findings open: a packaged `[[widget]]` couldn't be
added through the live builder (finding 7), a cell's `options.sceneId` didn't reach the widget (finding
8), the two live-only bundling bugs had no cheap guard (findings 5–6), and the widget cell rendered
visually blank (the parent-scope "fit per dashboard cell" risk). This session closes all of them —
verified LIVE, palette-driven, with the scene now framed in the cell.

## What shipped

**Finding 7 — restore the "Extension widgets" group in the reworked PanelEditor.** The viz panel-editor
rework's `QueryTab` source picker rendered only series/live/sql/extension; the `widget` PickerGroup (with
the finished packaged tiles) was dropped, so `extWidgetEntries` had no UI. Restored in
`ui/src/features/dashboard/editor/tabs/QueryTab.tsx`: the group is back in the source `<select>`; selecting
a tile sets `state.view = "ext:<id>/<widget>"` and clears the query target (a tile owns its data — no
`{tool,args}`); `entryId` resolves a widget cell back from `state.view`; switching datasource off a widget
cell drops the `ext:` view. This re-wires the EXISTING picker/serializer (the `view` round-trips verbatim
through `cellEditorState`) — **no new verb/cap/table/WIT**.

**Finding 8 — forward `cell.options`/`cell.binding` to the widget ctx.** `ExtWidget.tsx` hardcoded
`options:{}`/`binding:{}`, so the scope's intended `{view, options:{sceneId}}` never reached the tile
(Session 2 worked around it via a dashboard `const` var → `ctx.vars`). Now `WidgetView` passes
`cell.options`/`cell.binding` and `ExtWidget` forwards them as `ctx.options`/`ctx.binding` (re-mount keyed
on a `configKey` so a sceneId edit reframes). The **Scene picker** (finding 8's builder half):
`QueryTab` renders a scene `<select>` for a Scene tile, sourced from `useSceneDocs` (the shipped, ws-walled
`assets.list_docs`, `scene:` prefix), writing `cell.options.sceneId`. Verified live: a palette-built cell
saved `options.sceneId:"scene:ahu-1"` and the widget rendered from it (no `ctx.vars` needed).

**Findings 5–6 — cheap guards so a new bundling ext doesn't rediscover the live-only bugs.**
`rust/extensions/thecrew/ui/federation-remote.preset.ts`: a copyable preset carrying BOTH invariants —
`define: process.env.NODE_ENV` (Vite lib builds don't inject it → three.js "process is not defined") and
the React externals (single-copy via the shell import map). `vite.config.ts` now uses it. Finding 6: a
unit assertion in `mount.test.tsx` that the built `remoteEntry` exports a **`mount`** function (+ `mountWidget`
+ the default object) — fails fast without a browser (the Session-2 "remote does not export `mount`" bug).

**The blank widget-cell canvas — fit per dashboard cell.** The editor page frames a fixed ±350 world
units at zoom 1.6; a small read-only cell showed only a center crop → blank. New `canvas/fit-bounds.ts`
(pure math: scene bounds → fit zoom clamped to the page's [0.4,6] → box center) + `canvas/FitCamera.tsx`
(mounted inside `<Canvas>`, drives the ortho camera **every frame** so drei's declarative `makeDefault`
props can't clobber the fit — the one-shot-effect version stayed blank). `SceneCanvas` takes a `fit` prop
(the widget passes it; `CameraRig` omits MapControls in fit mode); the editor page is untouched. **Verified
live: the AHU-1 train renders framed + centered in the cell** (`docs/shots/scene-widget-dashboard.png`).

## Findings 9 note

Ran live on the **in-memory** engine (no `LB_STORE_PATH`), per finding 9 (SurrealKV `Invalid revision`).
No new persistence findings this session.

## Tests + green output

- thecrew UI unit (`rust/extensions/thecrew/ui`): **60/60** (was 57 pre-session) — +8 `fit-bounds.test.ts`,
  +3 `mount.test.tsx` export-contract. (One pre-existing unhandled `unicode-font-resolver` fetch in
  `scene-render.test.tsx` — a three.js text CDN fetch, unrelated; tests still pass.) `tsc --noEmit` clean.
- Dashboard-core unit (`ui/`): new `QueryTab.test.tsx` (5 — group offered, tile selection sets the view,
  scene picker sets `options.sceneId`) + `ExtWidget.test.tsx` (2 — options/binding reach ctx). The
  dashboard suite (widgetBuilder/cellEditorState/FlowsQuerySection) stays green (41/41 together).
- Gateway (real spawned node): `TheCrew.gateway.test.tsx` **6/6** (the mandatory capability-deny +
  workspace-isolation still deny/isolate), `panelEditor.gateway.test.tsx` **6/6**.
- **Live e2e** (real node in-mem :8080, built shell :4173, thecrew published + seeded):
  `ui/e2e/thecrew*.spec.ts` **2/2**. The widget spec now **drives the restored palette** — Add panel →
  pick "thecrew · Scene" → pick the AHU-1 scene → Save — instead of a seeded cell (finding 7 fixed), and
  the cell renders the fit scene (blank fixed). Screenshots refreshed: `docs/shots/scene-widget-dashboard.png`
  (palette-built, scene framed), `docs/shots/graphics-ahu-1-live.png` (page).
- Live publish/install: `make publish-ext EXT=thecrew` (204 as the member `user:ada` — the dev-login user
  isn't a member here) + `make seed-thecrew` (now also seeds an EMPTY `scene-build` dashboard the palette
  e2e builds onto). Bundle rebuilt via the preset: `process.env.NODE_ENV` fully replaced (0 occurrences),
  `mount`/`mountPage`/`mountWidget`/default all exported.

## Zero-core-additions held

Findings 7–8 re-wired the EXISTING dashboard picker/renderer + serializer (the `view`/`options` fields
already round-trip); the preset + fit code live entirely in the extension. No new verb/cap/table/WIT.

---

# Session 4 — Phase 3 (AI drawing): skill + teaching-error validation (+ the one core-blocked piece)

Phase 3 is "AI drawing": `skills/graphics-canvas/SKILL.md`, teaching-error validation, a draw-with-AI
rail, and the channel rich-response embed. The AI-drawing LOOP is inherently zero-core (the agent edits
the scene doc with the same shipped `assets.*` verbs), so most of the phase shipped; ONE piece — the
in-page rail — is genuinely blocked on a core addition and was STOP-and-surfaced.

## What shipped

- **`docs/skills/graphics-canvas/SKILL.md`** — the agent-drivable surface: auth, the three `assets.*`
  verbs + the `scene:` id / `content_type:"json"` / `tags:["scene"]` conventions, the scene schema, the
  **shape catalog** (generated from the live registry), the `bind[slot]={channel:"<series>"}` contract,
  the read-modify-save loop with self-correction via the teaching report, a **worked "draw AHU-1" run**
  (real curl), the channel/dashboard embed payload, and the capability/isolation notes. Consumed
  server-side via `agent::invoke`'s `skill` param (channels / workflow triage / ACP session).
- **Teaching-error validation** — `scene/catalog.ts` turns the renderer's `SymbolDef` registry into a
  compact catalog (`describeCatalog`/`knownTypes`/`catalogText`) — ONE source of truth, no
  hand-maintained second list (a cross-check test asserts it matches `ShapeNode.SYMBOLS`). `validate.ts`
  now flags an **unknown type** as an `unknownType` issue (still renders a placeholder — never a crash)
  and `teachingReport(issues)` returns every issue + the catalog when a type is wrong, so an AI mid-draft
  reads exactly what to fix and re-saves in one step (parent scope's "errors must teach"). Broke a cycle
  building this: `ShapeNode → scene-store → validate → catalog` — `catalog.ts` imports the shape `*Def`s
  directly (they depend only on `shape-props`), not `ShapeNode.SYMBOLS`.
- **Channel / dashboard embed — verified shipped, no new code.** A rich-response
  `{view:"ext:thecrew/scene", options:{sceneId}}` builds a v2 cell (`ResponseView.buildCell`, which
  forwards `options`) → `WidgetView` → `ExtWidget`, whose finding-8 wiring now delivers `ctx.options.sceneId`.
  Locked with a `ResponseView.test.tsx` case (the scene-embed contract) — the same path a dashboard cell
  uses, so a graphic drops into a thread live.

## STOP-and-surfaced (core-blocked): the in-page draw-with-AI RAIL

An "ask the canvas to draw" rail that the extension page fires itself CANNOT be built zero-core:
`agent.invoke` is **not an MCP tool** (`crates/host/src/agent/tool.rs` dispatches only
`agent.policy.set`/`decide`/`runtimes`/`config`; `agent::invoke` is a Rust entry with no `/mcp/call` arm
and no gateway route; the shell's `agent_invoke` command isn't mapped in the browser HTTP transport), and
the extension page bridge speaks only `/mcp/call` over `cell.tools ∩ grant`. The rail needs a NEW surface
(expose `agent.invoke` as an MCP verb, or a `/agent/invoke` gateway route). Surfaced in the extension
scope §Risks; NOT built. Interim: drive the agent through an existing surface (channel / composer) with
the `graphics-canvas` skill — the open canvas re-renders on each save.

## Tests + verification

- thecrew UI unit: **66/66** (+6: `catalog.test.ts` 4, `validate.test.ts` +2 teaching cases). `tsc` clean.
  The unknown-type test was updated from "reports nothing" to "TEACHES" (the intended phase-3 behavior).
- Dashboard-core unit: `ResponseView.test.tsx` +1 (the ext-widget scene-embed options contract) → 5/5.
- **Live recipe check:** ran the SKILL §6 read-modify-save loop verbatim against the real node
  (`assets.put_doc` → `get_doc` parses back the shapes → `list_docs` discovers it by the `scene:` prefix)
  — the skill's instructions work as written (the honest stand-in for an LLM agent-path test, which needs
  the invoke surface above).

## Not built (parent-scope phases 4–5)

Symbol packs (pack manifest + loader + `hvac` starter pack) and 3D (persp camera, GLTF import, extrusion,
status-bound materials). Plus the draw-with-AI rail (above) once the agent-invoke surface exists.
