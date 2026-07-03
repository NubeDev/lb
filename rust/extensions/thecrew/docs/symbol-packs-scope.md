# thecrew scope — symbol packs: new symbols as data, authorable by AI at runtime

Status: scope (the ask). This is the **implementation scope for graphics-canvas
phase 4** (`docs/scope/frontend/graphics-canvas-scope.md`, repo root — the parent
stays authoritative). Promotes into `docs/public/frontend/graphics-canvas.md` on ship.

Today a new equipment symbol is a hand-written React component (`ui/src/canvas/shapes/`,
`symbols-scope.md`) — adding a chiller means a code change, a build, and a re-publish.
The ask: **a new symbol becomes a workspace document.** A *symbol pack* is data — a
manifest plus per-symbol definitions in a small declarative **parametric part spec** —
interpreted at runtime by one generic renderer component. A human can write one by hand;
**an AI agent can author one in the browser session, at runtime**, through the same
shipped `assets.*` verbs it already uses to draw scenes. No code deploy, no eval, no new
core surface.

## Goals

- **Pack format v1** — versioned, additive-only from day one (it's a public contract
  humans and AIs author): a manifest (`id`, `name`, `v`, symbol map) + one definition
  per symbol.
- **The parametric symbol spec** — each symbol is data:
  - **parts**: a closed vocabulary of geometry primitives (`box`, `cylinder`, `plane`,
    `path` (2D polyline/outline, extrudable), `text`, `asset` (a GLTF/SVG workspace
    asset by id)) with transforms and token-referenced materials;
  - **params**: the symbol's prop schema (≤8 props, same `SymbolDef` shape as builtins)
    feeding part dimensions through **affine expressions only**
    (`{"prop":"w","mul":0.5,"add":-10}`) — no general expression language;
  - **anchors**: named position + direction (affine in params), the snap/connect
    contract from `builder-ux-scope.md`;
  - **bind slots** wired to a **closed behavior catalog** — `spin`, `sweep`,
    `tint-ramp`, `status-emissive`, `flow-chevrons`, `billboard` — behaviors are code,
    shipped once inside the interpreter; a symbol composes them by name. This is how
    data gets motion without executing data.
- **One interpreter** (`PackSymbol.tsx` + one file per part kind / behavior) merged
  into the existing registry: pack types show in the palette, dispatch through
  `ShapeNode`, appear in `catalog.ts` (so the teaching validator covers them), and
  edit in the PropertyRail — indistinguishable from builtins to the rest of the app.
- **Packs are workspace docs**: `assets.put_doc`/`get_doc`/`list_docs` with the id
  prefix **`pack:`** (the shipped `scene:` convention); binary parts (GLTF/SVG) ride
  `assets.put_asset`/`get_asset`. Missing pack → the shipped labeled placeholder,
  never a crash.
- **AI authoring as a first-class producer**: `docs/skills/graphics-canvas/SKILL.md`
  gains an "author a symbol pack" section (part/behavior catalog + a worked "make me a
  chiller symbol" run), and pack validation returns **teaching errors** (the failing
  part + the catalog of what *was* legal — the `catalog.ts`/`teachingReport` pattern).
- **Dogfood**: re-express the HVAC starter six (`symbols-scope.md`) as the first pack.
  Exit gate: the AHU-1 hero shot renders from the pack with no visible regression
  against `docs/shots/`.

## Non-goals

- **No runtime code generation.** The AI never emits TSX/JS that the browser compiles
  or evals — see Intent for why this is rejected, hard.
- **No code-level shape plugins** (parent scope stance unchanged): stays a non-slice
  until a pack provably can't express a needed symbol — that's a *finding*, not a
  license to eval.
- **No new wire protocol or dependency.** `json-render` and A2UI were evaluated as
  priors for this exact problem (AI-authored UI from a validated catalog): the
  **pattern is adopted** — host-owned catalog, AI emits schema-constrained JSON,
  validate-before-render, teach on failure — the **dependencies rejected**
  (`json-render` renders DOM-React trees against its own schema, a second schema next
  to the scene document; A2UI is a cross-platform app-UI protocol, and the parent
  scope already rejected a streaming draw channel).
- **No pack signing/marketplace.** Packs are workspace data behind the workspace wall,
  not registry extensions; cross-workspace sharing is a later ask.
- **No in-page "make me a symbol" rail** — blocked on the same agent-invoke surface
  finding as the draw-with-AI rail (parent scope Open question 6). Interim: ask the
  agent through a channel; the open palette refreshes on pack save.
- **No AI-authored GLTF.** The AI writes parametric parts; GLTF/SVG parts exist for
  human-imported geometry.

## Intent / approach

**The key idea: extend the pattern that already works one level down.** The scene
document already proves that a validated, catalog-constrained JSON document is
something an LLM can author safely at runtime. A symbol pack is the same move applied
to the catalog itself: the *set of types* becomes a document too, and the only new
code is one interpreter that turns part-lists into meshes.

```
pack document (assets.*, `pack:` prefix, workspace-walled)
   ▲ written by                            ▼ read by
   ├── AI agent (SKILL-guided, teaching     ├── palette / catalog.ts / PropertyRail
   │   validation, same assets verbs)       │     (merged SymbolDefs — builtins + packs)
   └── human (hand-written JSON)            └── PackSymbol.tsx (parts → meshes,
                                                  behaviors → animation, tokens → materials)
```

A symbol definition (the reviewable core of this scope):

```jsonc
// pack manifest: doc id "pack:chw", content_type json, tag "pack"
{
  "v": 1, "id": "chw", "name": "Chilled water plant",
  "symbols": {
    "chiller": {                       // scene shapes reference type "chw.chiller"
      "label": "Chiller",
      "props": { "w": { "kind": "number", "default": 160 },
                 "label": { "kind": "text", "default": "CH-1" } },
      "bindSlots": ["status", "load"],
      "anchors": [
        { "name": "chw_out", "pos": [{ "prop": "w", "mul": 0.5 }, 0, 0], "dir": [1, 0, 0] },
        { "name": "chw_in",  "pos": [{ "prop": "w", "mul": -0.5 }, 0, 0], "dir": [-1, 0, 0] }
      ],
      "parts": [
        { "kind": "box", "size": [{ "prop": "w" }, 90, 60], "material": "equipment",
          "behavior": { "status-emissive": { "bind": "status" } } },
        { "kind": "cylinder", "r": 28, "h": 60, "at": [0, 0, 34], "material": "rotor",
          "behavior": { "spin": { "bind": "load", "axis": "z" } } },
        { "kind": "text", "text": { "prop": "label" }, "at": [0, -70, 0] }
      ]
    }
  }
}
```

Materials are **token names only** (`theme/materials.ts` resolves them — the
playground's "a symbol never picks its own colors" rule survives the format). Every
part renders under both cameras — one geometry, two views, exactly like builtins — so
the design language's flat/3D duality is carried by the part vocabulary itself, not by
per-symbol code.

**Why data, not code — the rejected alternative, explicitly.** Compiling AI-emitted
TSX in the browser (sucrase/esbuild-wasm → blob `import()`) is technically feasible
and was rejected: it is arbitrary unsigned code running in the user's session, which
breaks the capability-first / signed-registry posture (rule 5 and the registry's
verify-before-store exist precisely so nothing unreviewed executes); it is
un-validatable (code that compiles can still wreck the scene, leak the bridge token,
or wedge the render loop); and it is un-teachable (a thrown exception can't return
"here is the catalog of what was legal"). Data is validatable *before* it renders,
budgetable (part/vertex caps), diffable, and undoable. If a symbol truly needs code,
it goes through the normal extension pipeline (a coding-workflow job → review → signed
publish), not runtime eval.

**Also rejected:** a general expression language in the spec (a mini-eval is eval;
affine `mul`/`add` covers dimension-from-prop, which is what the starter six actually
need) and GLTF-only packs (opaque to an AI author, and carrying no prop/anchor/bind
semantics — GLTF stays a *part kind*, not the format).

**File layout (new code only; FILE-LAYOUT applies):**

```
ui/src/packs/
├── pack.types.ts        # pack manifest + symbol/part/anchor spec (the format contract)
├── validate-pack.ts     # total validation + teaching report (part/behavior catalog on error)
├── load-packs.ts        # list_docs `pack:` → parse → validate → merged SymbolDef registry
├── affine.ts            # the {prop,mul,add} resolver (bounded, total)
├── PackSymbol.tsx       # the interpreter: def + props + bound values → parts → JSX
├── parts/               # one part kind per file: box.tsx cylinder.tsx plane.tsx path.tsx text.tsx asset.tsx
└── behaviors/           # one behavior per file: spin.ts sweep.ts tint-ramp.ts status-emissive.ts flow-chevrons.ts billboard.ts
```

`catalog.ts` merges pack entries into the existing `DEFS` flow (same one-source-of-truth
rule; builtin type prefixes `hvac.`/`plan.`/`shape.` are **reserved** — a pack claiming
them fails validation with a teaching error).

## How it fits the core

- **Tenancy / isolation:** packs are docs/assets in the caller's workspace; every
  read/write crosses the bridge with workspace from the signed token. A ws-B scene
  referencing a ws-A pack type renders the placeholder (the pack is simply absent).
- **Capabilities:** consumes the already-granted `assets.get_doc`/`put_doc`/`list_docs`;
  **grant delta:** the manifest adds `mcp:assets.put_asset:call` + `mcp:assets.get_asset:call`
  for GLTF/SVG parts (page scope; the widget stays read-only and gains only `get_asset`).
  Deny paths: pack save without the grant → surfaced deny; a viewer who can't read the
  pack doc → placeholders, never a crash.
- **Placement:** either — docs + bridge identical edge/cloud; packs work offline.
- **MCP surface:** **none added** — pure consumer of shipped verbs (reads =
  `get_doc`/`list_docs`/`get_asset`; write = `put_doc`/`put_asset`; no live feed —
  palette refresh rides the existing repaint path; no batch — a pack is one doc,
  bounded, synchronous).
- **Data (SurrealDB):** no new tables; pack = a doc record, models/SVGs = assets.
  State only — nothing pack-shaped moves on the bus.
- **Stateless extension:** the interpreter is a pure render of (pack defs, shape
  props, bound values); hot-reload safe.
- **No mocks:** tests seed real pack docs + real scenes + real series through the real
  gateway (`pnpm test:gateway`); no `*.fake.ts`.
- **SDK/WIT impact:** none — UI-only, stub component unchanged.
- **One responsibility per file:** the layout above; one part kind / one behavior per file.
- **Skill doc:** **yes** — this is an agent-drivable surface. The implementing session
  extends `docs/skills/graphics-canvas/SKILL.md` with the pack-authoring section
  (format, part/behavior catalog, teaching-error loop, a worked live run). A stale
  skill here is a finding.

## Example flow — "AI, we need a chiller symbol"

1. User (in a channel): *"Make a chiller symbol and add two chillers to the plant
   page, bound to `chw.ch1.*` / `chw.ch2.*`."*
2. The agent (SKILL-guided) `get_doc`s `pack:chw` (absent → starts from the skill's
   template), writes the `chiller` definition above, `put_doc`s it back. Capability +
   workspace checked at the host, as ever.
3. It validates by re-reading and running the documented check; a bad part kind came
   back as a teaching error naming the part index and the legal part catalog — one
   self-correction pass, by design.
4. The agent then edits `scene:plant` (the shipped drawing loop): two shapes of type
   `chw.chiller` with `bind` blocks, saved via `put_doc`.
5. The user's open canvas re-renders: `load-packs.ts` merges the new pack, the palette
   grows a "Chilled water plant" group, both chillers render with live status
   emissives and load-driven rotor spin. Nothing was compiled, evaled, or published.
6. The user drags a third chiller from the palette by hand — same symbol, same rail,
   same anchors. Human and AI authored surfaces are the same surface.

## Testing plan

Per `docs/scope/testing/testing-scope.md` — mandatory categories first:

- **Capability deny (real gateway):** pack save without `assets.put_doc` → surfaced
  deny; GLTF part upload without `put_asset` → deny; viewer without `get_doc` on the
  pack → scene renders placeholders.
- **Workspace isolation:** `pack:chw` saved in ws A invisible to ws B via
  `list_docs`/`get_doc`; ws B's scene using `chw.chiller` renders the placeholder.
- **Unit (vitest):** `validate-pack.ts` (unknown part kind, reserved prefix, over-budget
  parts, bad affine ref, missing anchor dir — each returns a teaching report);
  `affine.ts` bounds; registry merge (builtin collision rejected, pack entries appear
  in `describeCatalog()`).
- **Render (r3f test renderer, no pixels in CI):** the chiller def builds the expected
  scene graph; each part kind renders; each behavior wires to its bound value; unknown
  part kind inside an otherwise-valid symbol renders that part as placeholder geometry.
- **Integration (`pnpm test:gateway`):** seed pack doc + scene + series → load →
  palette merge → render path asserts bound values reach behaviors; **agent-path
  test:** drive the author-validate-fix loop through the real MCP surface (submit a
  broken def, assert the teaching report, submit the fix, assert the merged catalog).
- **Hot-reload / pack-reload:** re-save the pack doc; an open canvas re-renders the
  new geometry without remount errors (stateless).
- **Dogfood parity (manual, mandatory):** the HVAC-six pack renders the AHU-1 hero
  shot; screenshot into `docs/shots/` beside the builtin-render baseline.

## Risks & hard problems

- **The expressiveness cliff.** Some symbol will not be expressible in parts + affine
  exprs + six behaviors (the duct's styled corners and the door's wall-cutting are the
  starter set's hard cases). The discipline: render the nearest honest approximation
  or the placeholder, file the missing part/behavior as a finding, extend the closed
  vocabulary **additively** — never reach for eval. Budget the dogfood phase to
  surface these early.
- **AI-emitted garbage at a new layer.** A pack def can be structurally valid and
  visually terrible, or huge. Hard caps in `validate-pack.ts` (max symbols/pack, max
  parts/symbol, max path points, max text length), rejected with teaching errors —
  and the flat-mode screenshot stays the judge of "good", which no validator provides.
- **Two documents, one meaning.** A scene references pack types by name; renaming or
  deleting a symbol orphans shapes into placeholders. Acceptable v1 behavior (it's
  visible and non-fatal), but the SKILL must teach "additive edits, don't rename" —
  and last-writer-wins on `put_doc` applies to packs exactly as it does to scenes
  (same interim mitigation, same `document-store/` revision ask).
- **Doc-size ceiling.** A pack with many symbols is one JSON string through
  `put_doc`; verify the practical content-size limit early and state a per-pack
  symbol budget honestly rather than discovering truncation in the field.
- **Interpreter performance.** Builtins are hand-tuned meshes; a naive interpreter
  rebuilding geometry per frame will not hold 60fps on a 200-shape page. Memoize
  geometry per (def, resolved params); behaviors mutate materials/rotation only.

## Open questions

1. **Affine-only expressions: enough?** The dogfood answers it. If the starter six
   need more, the next step is a *small closed function set* (`min`/`max`/`clamp`),
   not a grammar. Decide from evidence, not anticipation.
2. **Full builtin replacement or coexistence?** Leaning coexistence: builtins stay
   (they're the fallback and the perf baseline), the pack proves the format, and
   builtin retirement is a later cleanup once parity is proven by screenshot.
3. **Flat glyph override:** does any symbol need a distinct 2D representation
   (`partsFlat`), or does one geometry under two cameras hold, as it did for the
   playground set? Leaning no override until a dogfood symbol proves the need.
4. **GLTF part binding:** when a part is an asset, do behaviors target named GLTF
   nodes (`"node": "impeller"`)? Defer until the first real GLTF pack; parametric
   parts don't need it.
5. **Palette grouping:** one collapsible group per pack (leaning yes — pack `name` is
   the group label), and where builtins sit relative to packs.

## Related

- `docs/scope/frontend/graphics-canvas-scope.md` (repo root) — the parent; this is
  its phase 4, and inherits its symbol-packs-as-data decision and A2UI/Awaken verdicts.
- `thecrew-extension-scope.md` (this folder) — phases 1–2 (shipped); the `scene:`
  id-prefix convention and the last-writer-wins finding this scope inherits.
- `symbols-scope.md` — the hand-written starter set this format must re-express;
  `look-scope.md` / `builder-ux-scope.md` — the visual + anchor contracts.
- `docs/skills/graphics-canvas/SKILL.md` — gains the pack-authoring section (owned by
  the implementing session, grounded in a live run).
- `docs/public/frontend/graphics-canvas.md` — where this promotes on ship.
- Evaluated priors: `vercel-labs/json-render`, A2UI — pattern adopted, dependency
  rejected (see Non-goals). README §3 rules 4, 5, 8, 9.
