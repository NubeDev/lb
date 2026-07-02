# Graphics canvas

A free-form, data-bound graphics surface (plant graphics / floor plans; 3D later) shipped as a
**100% UI extension** — the `thecrew` extension at `rust/extensions/thecrew/`. One engine (three.js
via `@react-three/fiber`, flat now, 3D from the same scene document later), zero core additions: a
pure consumer of shipped `assets.*` + `series.*` verbs through the host-mediated bridge.

**Shipped: phases 1–2 (viewer + editor).** The ask: `../../scope/frontend/graphics-canvas-scope.md`
(authoritative for schema/engine/phases 3–5); the phases 1–2 build scope + findings co-locate at
`rust/extensions/thecrew/docs/thecrew-extension-scope.md`; the build session is
`../../sessions/frontend/thecrew-extension-session.md`.

## What it is

- **A publishable, installable extension** (proof-panel packaging): a zero-tool wasm32-wasip2
  component (`src/lib.rs`) that satisfies the loader + registry publish path (there is no UI-only
  tier), plus a federated UI bundle. All real behaviour is in `ui/`.
- **Two mounts from one `remoteEntry.js`:** a full graphics **page** (`[ui]` — palette + canvas +
  property rail + a scene picker/save bar) and a read-only **scene cell** (`[[widget]]`) for
  dashboards. The widget's grant is deliberately narrower — it can render a scene and its live
  values but can never save one.
- **Scenes are workspace docs.** Load/save/list through `assets.get_doc` / `assets.put_doc` /
  `assets.list_docs`. A scene doc is JSON (`content_type: "json"`), owned by the caller, walled by
  workspace. Discovery convention: the doc id carries the `scene:` prefix (the picker filters on it)
  and the `scene` tag.
- **Live values through the bridge.** A shape prop binds to a series channel; one ValueSource
  multiplexer collects + dedupes every bound channel, backfills with `series.latest`, and streams
  live via `series.watch` (widget tier) or polls `series.latest` (call-only page bridge). Bindings
  run under the **viewer's** grant — a denied series renders the shape's no-access state, never a
  crash; a denied save surfaces the deny honestly.

## Persistence & concurrency (interim)

`assets.put_doc` is **last-writer-wins today** — no revision check. The interim (client-side): a
whole-doc read-before-write compare against the snapshot the editor loaded; a mismatch surfaces an
honest "scene changed underneath you — reload?" prompt instead of silently clobbering. The real fix
is a generic `document-store/` revision ask (`put_doc` gaining an optional `expected_rev`), not a
thecrew workaround.

## Capabilities

Page scope: `assets.get_doc`, `assets.put_doc`, `assets.list_docs`, `series.latest`, `series.read`,
`series.watch`. Widget scope: `assets.get_doc`, `series.latest`, `series.watch` (no save/list). Each
is intersected with the admin's install grant and re-checked host-side per call, workspace-first.
Note: a real save needs the install grant to carry `mcp:assets.put_doc:call` (the default member cap
set does not include it).

## Not yet (parent-scope phases 3–5)

AI drawing + `skills/graphics-canvas/SKILL.md`, symbol packs (equipment as data), and 3D-first work.
Shape `action` execution (click-to-command) and multi-user co-editing are non-goals for phases 1–2.
