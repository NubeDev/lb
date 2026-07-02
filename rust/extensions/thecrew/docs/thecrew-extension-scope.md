# thecrew scope — the lift: playground → LB extension (graphics-canvas phases 1–2)

Status: **built & tested (2026-07-02)** — phases 1–2 shipped; promoted to
`docs/public/frontend/graphics-canvas.md`. Build session:
`docs/sessions/frontend/thecrew-extension-session.md` (green output there). Findings surfaced
during the build are folded into Open questions + Risks below (no core additions were made).
This is the **implementation scope** for the first two phases of
`docs/scope/frontend/graphics-canvas-scope.md` (repo root) — that doc stays the
authoritative feature scope (schema, engine decision, symbol packs, AI drawing);
this one scopes only *turning the proven playground into a real extension*.

The playground proved the look and the builder (see `thecrew-scope.md` and
`docs/sessions/frontend/thecrew-session.md`: both demos render, 37/37 vitest, the
reuse contract held). The package now lives at `rust/extensions/thecrew/` and the
ask is: make it a **publishable, installable LB extension** — a federated `[ui]`
page + a `[[widget]]` scene cell, scenes persisted as workspace docs, values bound
through the host-mediated bridge — with **zero core additions**.

## Goals

- **A real extension artifact**: `extension.toml` + a signed publishable bundle that
  installs, loads, and mounts through the shipped registry → federation path, exactly
  like `proof-panel`.
- **The seam swap** the playground was built for: `data/value-source.ts` stops being
  fed by the simulator and is fed by the **bridge** (`series.latest` backfill +
  `series.watch` SSE) under the viewer's grant.
- **Scenes become workspace state**: load/save through the shipped `assets.get_doc` /
  `assets.put_doc` / `assets.list_docs` verbs (this answers the parent scope's Open
  question 1 — the assets surface is shipped; no `kv.*` fallback needed).
- **Both mounts from one build**: the full-bleed graphics page (`[ui]`, palette +
  canvas + rail — the whole playground app) and the read-only scene viewer cell
  (`[[widget]]`) as a second export on the same `remoteEntry.js`.
- Everything under `ui/src/{scene,canvas,theme,editor,state}` lifts **unchanged**
  (the playground's reuse contract §1–5); the new code is the mount shell, the
  bridge value source, and scene I/O.

## Non-goals

- **No AI drawing, no symbol packs, no 3D-first work** — graphics-canvas phases 3–5
  stay in the parent scope (the skill doc `skills/graphics-canvas/SKILL.md` lands
  with phase 3, not here).
- **No new core verbs, tables, capabilities, or WIT surface.** If the lift appears
  to need one, that's a finding to surface in the parent scope.
- **No shape `action` wiring yet** (click-to-command) — the document schema carries
  it; execution lands with phase 3's write story.
- No multi-user co-editing; concurrency is honest-but-simple (see risks).

## Intent / approach

**Does it need Rust code? Yes — but only a stub.** The manifest loader
(`lb-ext-loader`) has exactly two tiers (`wasm` | `native`), and the registry's
`Artifact`/publish path requires component bytes (`lb-registry` `Artifact.wasm`,
verify-before-store). There is no UI-only tier, and adding one would be core surface
— rejected per the zero-core-additions posture; a zero-tool wasm component is ~20
lines and free at runtime. So `src/lib.rs` is a **Tier-1 wasm component that serves
no tools** (proof-panel minus the tools): it satisfies the world, publishes, and
loads; all real behavior lives in `ui/`. Rejected alternative: keeping thecrew
outside the extension system as a standalone app — it would never inherit the cap
gate, workspace wall, or install lifecycle, which is the whole point of the lift.

The extension keeps the **id `thecrew`** (the directory the code already lives in).
The parent scope imagined `rust/extensions/graphics-canvas/`; we keep the working
name and update the cross-references instead of churning the tree. Note: the id
leaks into the served UI route (`/extensions/thecrew/ui/…`) and install records, so
if it's ever renamed, rename **before first publish** (Open question 1).

```
rust/extensions/thecrew/
├── extension.toml            # manifest below
├── build.sh                  # wasm component + federated UI bundle (proof-panel pattern)
├── Cargo.toml                # own workspace (excluded from host workspace, like proof-panel)
├── src/lib.rs                # the stub component: implements the world, zero [[tools]]
├── docs/                     # this scope + the playground scopes (history) + shots/
└── ui/
    ├── vite.config.ts        # lib build → dist/remoteEntry.js (federation remote)
    └── src/
        ├── mount.tsx         # mountPage(el, ctx, bridge) + mountWidget(el, ctx, bridge, id)
        ├── bridge/
        │   ├── scene-io.ts   # load/save/list scenes via assets.* over the bridge
        │   └── bridge-source.ts  # ValueSource impl: series.latest backfill + series.watch
        ├── scene/ canvas/ theme/ editor/ state/   # lifted unchanged from the playground
        └── data/value-source.ts                    # the seam (unchanged interface)
```

**The simulator does not lift.** It was the playground's one declared fake, allowed
only because there was no node there at all. Inside the extension the no-mocks rule
(CLAUDE §9) applies in full: `simulator.ts` is deleted in the lift; demos and tests
get their values from **real seeded series** (`ingest.write` through the real
gateway, the proof-panel pattern) and their scenes from **real seeded docs**
(`assets.put_doc`).

**Manifest sketch** (the reviewable core of this scope):

```toml
[extension]
id          = "thecrew"
version     = "0.1.0"
name        = "Graphics"
description = "Plant graphics & floor plans: a data-bound three.js scene canvas (graphics-canvas phases 1-2)."

[runtime]
tier      = "wasm"                            # stub component, zero tools
world     = "lazybones:ext/extension@0.2.0"
placement = "either"                          # assets + bridge identical edge/cloud

[capabilities]
request = [
    "mcp:assets.get_doc:call",     # load a scene
    "mcp:assets.put_doc:call",     # save a scene (editor)
    "mcp:assets.list_docs:call",   # the scene picker
    "mcp:series.latest:call",      # binding backfill
    "mcp:series.read:call",        # sparkline/history bindings (label trends)
    "mcp:series.watch:call",       # live values via the shipped SSE
]

[ui]              # the full graphics page: browse/edit/create scenes
entry = "remoteEntry.js"
label = "Graphics"
icon  = "shapes"
scope = ["assets.get_doc", "assets.put_doc", "assets.list_docs",
         "series.latest", "series.read", "series.watch"]

[[widget]]        # the read-only scene cell for dashboards
entry = "remoteEntry.js"
label = "Scene"
icon  = "shapes"
scope = ["assets.get_doc", "series.latest", "series.watch"]

[visibility]
class = "public"
```

The widget's scope is deliberately narrower than the page's: a dashboard cell can
render a scene and its live values but can never save one.

## How it fits the core

- **Tenancy / isolation:** scenes are docs in the caller's workspace; every
  read/write crosses the bridge with workspace resolved from the signed token.
  Nothing thecrew-specific to enforce; isolation still tested (below).
- **Capabilities:** the six read/write caps above, intersected with the admin's
  install grant; bindings run under the **viewer's** grant. Deny paths: a bound
  shape renders its no-access state (never a crash); a denied save surfaces the
  deny honestly in the toolbar.
- **Placement:** either — no `if cloud`, works offline against a local node.
- **MCP surface:** **none added** — pure consumer of shipped verbs (the parent
  scope's API-shape answer: reads = `assets.get_doc`/`list_docs` + series reads;
  write = `assets.put_doc`; live feed = `series.watch`; no batch verbs).
- **Data (SurrealDB):** no new tables; scene = a doc record (`content_type`
  JSON), owner forced from the principal by `put_doc`.
- **Bus (Zenoh):** nothing new; live values ride the existing series SSE.
- **State vs motion:** the scene document is state (a doc); values are motion
  (watch). The playground's undo stack stays client-side (motion-free).
- **Stateless extension:** the page/widget is a pure render of (doc, bound
  values, local edit state); hot-reload safe.
- **No mocks:** the simulator is deleted; tests seed real docs + real series and
  run against the real spawned gateway (`pnpm test:gateway`).
- **SDK/WIT impact:** none — stub component on the existing `@0.2.0` world.
- **One responsibility per file:** the lift preserves the playground layout
  (already FILE-LAYOUT-clean); new files are one seam each (`mount.tsx`,
  `scene-io.ts`, `bridge-source.ts`).
- **Skill doc:** N/A for this lift — the agent-drivable surface (AI drawing) is
  phase 3 in the parent scope, which names `skills/graphics-canvas/SKILL.md`.

## Example flow

1. `./build.sh` → stub component + `ui/dist/remoteEntry.js`; publish the signed
   artifact (`ext.publish`); admin installs with the requested grant.
2. The shell shows **Graphics** in the nav (cap-gated slot); the page lists scenes
   via `assets.list_docs` (tag `scene`), user opens the seeded AHU-1 demo.
3. `scene-io.ts` loads the doc; `validate.ts` normalizes (unknown type →
   placeholder, unchanged from the playground); `<SceneCanvas>` renders flat mode.
4. `bridge-source.ts` collects the doc's `bind` channels, backfills with
   `series.latest`, subscribes via `bridge.watch("series.watch")` — SF-1 spins at
   its real bound rpm from the seeded series.
5. User drags a coil in from the palette, tunes it in the rail, hits save →
   `assets.put_doc` re-checked at the host, workspace-first.
6. A dashboard adds the **Scene** widget pointing at the same doc id — read-only
   render, live values, no save button; a viewer without `series.read` on those
   series sees the shapes' no-access state.

## Testing plan

Per `docs/scope/testing/testing-scope.md` — mandatory categories first:

- **Capability deny (real gateway):** save without the doc-write grant → surfaced
  deny; widget viewer missing `series.watch` → no-access state on bound shapes,
  scene still renders.
- **Workspace isolation:** a scene doc saved in ws A is invisible to ws B via
  `list_docs`/`get_doc`; ws B's watch cannot reach ws A's series.
- **Unit (lifted, stays green):** the playground's 37 vitest tests (validate,
  defaults, snap, undo, store, r3f scene-graph render) minus the simulator suite,
  re-pointed at static resolved values through the ValueSource seam contract.
- **Integration (`pnpm test:gateway`):** seed a real scene doc + real series →
  load → edit → save → reload round-trip byte-stable; backfill-then-watch delivers
  a live sample to a bound prop; save surfaces `put_doc`'s result honestly.
- **Federation:** the page and the widget both load through the real
  `remoteEntry.js` route and mount in the real shell/`WidgetHost`.
- **Hot-reload:** re-publish; an open canvas remounts cleanly (stateless).
- **Render smoke:** scene-graph assertions (r3f test renderer), no pixels in CI;
  screenshots stay a local `verify` step into `docs/shots/`.

## Risks & hard problems

- **Last-writer-wins saves.** `assets.put_doc` has **no revision check** today
  (verified in `crates/host/src/assets/put_doc.rs`) — the parent scope assumed one.
  Two editors (or an editor + a future agent) can silently clobber each other.
  Interim: whole-doc read-before-write with a client-side content compare and an
  honest "scene changed underneath you — reload?" prompt. The real fix is a generic
  document-store revision ask (Open question 2), not a thecrew workaround.
- **Binding fan-out** (parent scope risk, now real): one multiplexer in
  `bridge-source.ts` — collect, dedupe, one watch per series, fan out. Budget it;
  per-shape subscriptions won't fly on a 200-prop page. **Built** — proven in
  `bridge-source.test.ts` (N subscribers → 1 backfill + 1 watch upstream).
- **Save needs an explicit install grant (finding, 2026-07-02).** The default member/dev cap
  set carries `store:doc/*:write` + `mcp:*.write:call` wildcards but **not** the exact verb cap
  `mcp:assets.put_doc:call` that `assets.put_doc`'s MCP gate 1 requires (`put_doc` isn't
  `*.write`). So a scene save works only because thecrew's install grant requests
  `mcp:assets.put_doc:call` — as the manifest does. A viewer without it is refused server-side
  (the deny test proves this). No core change; just note it for whoever grants the install.
- **`series.watch` live SSE has no gateway-vitest transport (finding).** The real-gateway
  harness has no watch path (matching proof-panel's live tile). The multiplexer's backfill
  (`series.latest`) is proven live in the gateway suite; the `watch` fan-out is proven via the
  widget stub (`bridge-source.test.ts`). A Playwright e2e is the honest place for live SSE — a
  deferred follow-up, not a phases-1–2 gap.
- **Bundle weight:** three.js rides only this remote (the federation payoff), but
  keep the shared-React import-map discipline from proof-panel so the remote doesn't
  double-load React.
- **WebGL contexts per dashboard cell** — inherit the parent scope's mitigation
  (render-on-demand, release offscreen); acceptable to defer past phase 1 with a
  documented cell cap.

## Open questions

1. **Final id: `thecrew` or `graphics-canvas`?** Decide before first publish (the
   id is in the UI route and install records). Default: keep `thecrew`.
2. **Doc revision checks:** raise the generic ask on
   `docs/scope/document-store/` — does `put_doc` grow an optional `expected_rev`?
   thecrew is the first real customer; file the finding, don't fork the verb.
3. **Scene discovery convention:** tag docs `scene` and filter `list_docs`, or a
   doc-id prefix (`scene:…`)? **RESOLVED (2026-07-02, build): id-prefix `scene:`.** The
   shipped `assets.list_docs` returns only `{id,title}` per doc — **no tags** (verified in
   `crates/host/src/assets/tool.rs`), so a tag-side filter is impossible without a core
   change. The picker filters on the `scene:` id prefix; docs are STILL tagged `scene` so a
   future tag-returning `list_docs` can filter server-side. (`bridge/scene-io.ts`.)
4. **Demo seeding:** seed the AHU-1 + floorplan demo docs at install time (a
   first-run seed from the page) or leave them to tests only? Leaning first-run
   offer ("create demo scenes"), never silent.

## Related

- `docs/scope/frontend/graphics-canvas-scope.md` — the parent feature scope
  (authoritative for schema/engine/phases 3–5); this doc is its phases 1–2 build.
- `thecrew-scope.md` + `look-scope.md` / `builder-ux-scope.md` / `symbols-scope.md`
  (this folder) — the playground scopes, now history + the visual bar.
- `docs/sessions/frontend/thecrew-session.md` — the playground build session.
- `rust/extensions/proof-panel/` — the packaging precedent (manifest, build.sh,
  `[ui]` + `[[widget]]` on one remote, gateway tests).
- `docs/scope/extensions/ui-federation-scope.md` — the mount + bridge contract.
- `docs/public/frontend/graphics-canvas.md` — the public stub this fills on ship.
- README §3 (rules 1–9), §6.12.
