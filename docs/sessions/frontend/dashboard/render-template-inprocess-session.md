# Session — render-template widget, in-process (no iframe)

- Scope: [`../../../scope/frontend/dashboard/render-template-inprocess-scope.md`](../../../scope/frontend/dashboard/render-template-inprocess-scope.md)
- Status: **shipped** (the `template` engine is in-process; `plot`/`d3` stay iframe). One host-side gap
  surfaced (rules-as-source *render* path) — out of this scope, tracked in
  [`../../../debugging/frontend/rules-as-source-render-path-empty.md`](../../../debugging/frontend/rules-as-source-render-path-empty.md).
- Date: 2026-07-05

## What shipped (the deliverable)

The eval-free `template` widget renders **in-process** (a first-class dashboard view, sibling of
`GenUiView`), no longer in the sandboxed iframe. Same data contract (`usePanelData`), same leashed +
host-re-checked write bridge; the sandbox is replaced by a **markup sanitizer** (DOMPurify, one file)
plus the existing `dashboard.save`/`template.save` authoring cap as the trust gate. `plot`/`d3` **stay**
on the iframe tier (they `eval` author JS — real RCE; the sandbox is load-bearing for them).

### Files (UI, all ≤ FILE-LAYOUT)

- **NEW** `ui/src/features/dashboard/builder/sanitizeTemplateHtml.ts` — the security boundary. Pure
  `string → string`; DOMPurify wrapped in ONE file with our config (conservative structural tag/attr
  allow-list; `data-call`/`data-args` admitted; `on*`/`<script>`/`<iframe>`/`<object>`/`<embed>`/
  `<link>`/`<meta>`/`<base>`/`javascript:`/non-image `data:` stripped; a `style`-scrubbing hook removes
  `expression()`/`-moz-binding`/`behavior` (dead in modern browsers but the scope requires it; jsdom's
  CSS parser doesn't enforce it, so the hook is load-bearing for the suite)).
- **NEW** `ui/src/features/dashboard/builder/sanitizeTemplateHtml.test.ts` — the **XSS-vector suite**
  (16 tests). This IS the security gate (replaces the sandbox). Covers mutation-XSS, svg/math namespace
  tricks, `data:`/`javascript:` URLs, all `on*` handlers, malformed/truncated input, idempotence, and
  never-throws. Green.
- **NEW** `ui/src/features/dashboard/builder/wireTemplateDataCalls.ts` — the imperative `[data-call]`
  post-commit wiring (Decision 5: reads ONLY `data-call`/`data-args`, never an author inline handler).
  Returns a cleanup fn (no listener leak / double-fire).
- **NEW** `ui/src/features/dashboard/builder/wireTemplateDataCalls.test.ts` — 7 tests: leash (in/out),
  data-*-only contract, malformed args, cleanup, multi-button.
- **NEW** `ui/src/features/dashboard/views/TemplateView.tsx` — the in-process view. `usePanelData` →
  `interpolateTemplate` (reused verbatim) → `sanitizeTemplateHtml` → `<div
  dangerouslySetInnerHTML>` in the shell widget chrome; post-commit effect wires `[data-call]` clicks
  to the leashed bridge. Inline-vs-Saved resolution lifted from `ScriptedView`.
- **NEW** `ui/src/features/dashboard/views/TemplateView.test.tsx` — 7 unit tests (thin IPC stub,
  rule-9-sanctioned transport shim, NOT a fake node): rows render, in-leash click forwarded, out-of-leash
  click rejected with NO invoke, denied source → standard denied panel, hostile `onerror` stripped by
  the full pipeline, Inline↔Saved both resolve, NO iframe mounted.
- **NEW** `ui/src/features/dashboard/views/templateView.gateway.test.tsx` — real-gateway tests. 5 pass,
  1 skip (rules — see "Host gap surfaced" below): series/SQL source renders 3 real rows in-process (no
  iframe), `[data-call]` write reaches the host (granted → ok), a write the principal lacks is denied
  at the host (guard 3 survives), workspace isolation (a `render_template` in ws-A invisible to ws-B),
  regression (`plot` still mounts the iframe).
- **NEW** `ui/src/features/panel-builder/tabs/options/TemplateOptionsEditor.tsx` — the "editable in Data
  Studio" glue. Bridges `EditorState.carry.extraOptions` ↔ `TemplateSourceField`'s `TemplateValue`.
  `code`/`templateId` ride `carry.extraOptions` verbatim (no serializer change).
- **EDITED** `ui/src/features/dashboard/views/WidgetView.tsx` — `case "template"` → `<TemplateView>`
  (was `<ScriptedView engine="template">`); `plot`/`d3` unchanged.
- **EDITED** `ui/src/features/dashboard/builder/iframeRuntime.ts` — **deleted** the dead in-frame
  `template` branch (Decision 4) + the `interpolateTemplate` embedding; narrowed the `engine` type to
  `"plot" | "d3"`. The sandbox now hosts only the two `eval`-based engines.
- **EDITED** `ui/src/features/dashboard/builder/WidgetIframe.tsx` + `ui/src/features/dashboard/views/
  ScriptedView.tsx` — `engine: "plot" | "d3"` (was `"plot" | "d3" | "template"`); header docs updated.
- **EDITED** `ui/src/features/dashboard/builder/trust.ts` — doc note: `template` is no longer a
  `scriptedTier` consumer; `scriptedTier()` governs `plot`/`d3` only.
- **EDITED** `ui/src/features/dashboard/builder/editors/CodeEditor.tsx` — `lang?: "javascript" |
  "html"` (template body uses the HTML grammar; `@codemirror/lang-html`).
- **EDITED** `ui/src/features/dashboard/builder/editors/TemplateSourceField.tsx` — `lang="html"` on the
  CodeEditor; header doc updated (renders in-process).
- **EDITED** `ui/src/features/panel-builder/tabs/PanelOptionsTab.tsx` — `case "template"` →
  `<TemplateOptionsEditor>` (the orphaned editor is wired).
- **EDITED** `ui/src/features/panel-builder/VizPicker.tsx` — added a **Template** entry (scope
  interpretation — see "Decisions I resolved during build" below).
- **EDITED** `ui/package.json` — `dompurify@^3.4.11` + `@codemirror/lang-html@^6.4.11`.

### Tests — green output

```
$ pnpm exec vitest run --config vite.config.ts \
    src/features/dashboard/builder/sanitizeTemplateHtml.test.ts \
    src/features/dashboard/builder/wireTemplateDataCalls.test.ts \
    src/features/dashboard/views/TemplateView.test.tsx
 ✓ sanitizeTemplateHtml.test.ts (16 tests)
 ✓ wireTemplateDataCalls.test.ts (7 tests)
 ✓ TemplateView.test.tsx (7 tests)
 Test Files 3 passed (3)  Tests 30 passed (30)

$ pnpm test    # full default suite
 Test Files 97 passed (97)  Tests 593 passed (593)

$ pnpm exec vitest run --config vitest.gateway.config.ts templateView.gateway
 ✓ templateView.gateway.test.tsx (6 tests | 1 skipped)
```

The full default suite (593) is green, including the existing `templateInterpolate.test.ts`
regression (the interpolator is reused verbatim — unchanged) and `widgetBuilder.test.ts` (the
trust-tier routing test).

## Decisions I resolved during build (the scope's two open questions + two interpretations)

**Open Q1 — the exact DOMPurify allow-list.** Resolved test-first: the XSS suite drove it. The config
keeps `data-call`/`data-args` (the write-button contract) + a conservative structural tag/attribute
set (`div/ul/li/table/button/img/…`, `class/style/href/src/…`) + `ALLOW_DATA_ATTR: true` (inert
`data-*` for CSS selectors). It strips every `on*` (a long explicit `FORBID_ATTR` list as
defense-in-depth on top of the allow-list), `<script>/<iframe>/<object>/<embed>/<link>/<meta>/<base>`,
and `javascript:`/script-bearing `data:` URLs (DOMPurify's default URI guard). A `style`-scrubbing
`afterSanitizeAttributes` hook removes `expression()`/`-moz-binding`/`behavior` — these are dead in
modern browsers but jsdom's CSS parser doesn't reject them, so without the hook the suite (correctly)
fails. The hook is idempotent and never throws.

**Open Q2 — does the shell CSP already forbid inline script (so Trusted Types is reachable)?**
Resolved: the shell has **NO CSP today** (no `Content-Security-Policy` meta in `index.html`). A
`require-trusted-types-for 'script'` directive is document-wide, so it cannot be added narrowly to the
`TemplateView` mount without a shell-wide CSP scope (and the shell's own inline bootstrap `<script>`
in `index.html` would violate a strict `script-src`). So Decision 5's Trusted-Types ceiling is
**deferred** — it needs its own shell-wide CSP scope. The **floor** (Decision 5) shipped: the
`data-*`-only click wiring + the sanitizer as the single sink + the XSS suite as the gate. Even a
hypothetical sanitizer miss has no inline-script sink in `TemplateView` (the wiring reads only
`data-call`/`data-args`, proven by `wireTemplateDataCalls.test.ts`).

**Interpretation A — `@types/dompurify` skipped.** The scope said "dompurify (+ @types/dompurify)".
DOMPurify 3.x **bundles its own types** (`./dist/purify.cjs.d.ts`); `@types/dompurify` is a v2-era stub
that conflicts with v3. The no-hack choice was to add `dompurify@^3.4.11` alone (its bundled types) and
skip `@types/dompurify`. Adding the stub would be wrong.

**Interpretation B — added a `Template` entry to `VizPicker`.** The scope's build order step 6 names
only `PanelOptionsTab`, and OUT-of-scope says "any new source or picker work" (meaning the
**source** picker / datasource). But "editable in Data Studio" is a deliverable, and without a viz-
picker entry a user can only ever EDIT pre-existing template cells — a dead authoring path. The genui
precedent (genui IS in the viz picker) settles this: a one-line `Template` entry makes the feature
genuinely reachable for a new cell. Mirrors `genui` exactly; not a hack.

## Host gap surfaced — rules-as-source RENDER path is empty (out of scope, tracked)

The scope's headline is "Rules (and every other source) work for free." Investigation against the real
gateway found this is **not true today for the render path**: a `template` (or chart/table) cell bound
to `{tool:"rules.run"}` renders **zero rows** through `viz.query`, even though the **direct**
`rules_run` route returns the rows (verified: `runRule({ruleId})` → `{kind:"scalar", value:[3 rows]}`
✓; the same rule via `viz.query` → `rows.length === 0` ✗). The gap is in `viz.query`'s recursive
dispatch of `rules.run` returning empty, **regardless of caps** — present for every view bound to
rules.run, not just the new template. The `RuleOutput` envelope (`{kind, value|columns+rows}`) is also
not unwrapped by `result_to_rows` (it checks `ROW_KEYS = [samples, items, rows, templates, dashboards,
reminders]`, none of which match `output`).

This is a **host-side pipeline gap**, explicitly outside this scope ("no change to the data pipeline",
"client-only render change"). Per the user's "surface contradictions" instruction, I did NOT fix it
(attempted a minimal `ROW_KEYS += "output"` first, reverted it — `output` is an envelope object, not a
bare array, so it's a no-op; the real fix needs the recursive-dispatch + envelope-unwrap
investigation). The gap has its own debug entry
([`../../../debugging/frontend/rules-as-source-render-path-empty.md`](../../../debugging/frontend/rules-as-source-render-path-empty.md))
and the gateway test's rules case is `it.skip` with a precise note. The in-process template view is
source-agnostic and renders whatever rows `usePanelData` resolves — proven by the series/SQL gateway
test (3 real rows) — so it will render rule rows the moment the host gap is fixed, with **no
template-side change**.

Because of this gap, I did **NOT** flip `rules-as-source-scope.md` to "shipped" (the brief said to,
but the brief assumed rules render; the picker is shipped, the render path is not). That doc's status
is updated to reflect the split: picker shipped, render path blocked.

## Capabilities + isolation (mandatory categories)

- **Capability deny** (mandatory, real gateway): a `[data-call]` write the principal LACKS is denied at
  the host — `data-called="err"` (the local leash lets it through because the author named the tool;
  guard 3, the host re-check, bites). Proven in `templateView.gateway.test.tsx`. Also unit-tested:
  out-of-leash `data-call` rejected locally with NO `invoke` (`wireTemplateDataCalls.test.ts` +
  `TemplateView.test.tsx`).
- **Workspace isolation** (mandatory, real gateway): a `render_template` saved in ws-A is invisible to
  ws-B (`template.get` rejects). The render path adds nothing new (the template reads only
  `usePanelData` frames, already workspace-scoped at the host). Proven in `templateView.gateway.test.tsx`.
- **No mocks / no fake backend** (rule 9): the unit tests use a thin IPC transport shim (the sanctioned
  pattern — `vi.mock("@/lib/ipc/invoke")` returning seeded rows, NOT a node re-implementation); the
  gateway tests drive a REAL spawned node seeded through the real write path.

## Regression coverage

- `templateInterpolate.test.ts` — unchanged, still green (the interpolator is reused verbatim).
- `plot`/`d3` STILL mount the iframe — `templateView.gateway.test.tsx` asserts `view:"plot"` mounts an
  `<iframe>` (the tier split is enforced).
- The iframe `template` engine branch is GONE (Decision 4); the `engine` type narrows to `"plot" |
  "d3"` so a future caller can't even pass `"template"` to the iframe path (the class of bug becomes
  unrepresentable — guardrail style).

## What's left (follow-ups, NOT this scope)

1. **The rules-as-source render path host gap** (above) — its own scope.
2. **A shell-wide CSP / Trusted Types posture** (Open Q2 deferral) — its own scope; the floor shipped.
3. **The doc-site (Nextra) + native desktop (webkit)** — unchanged, still the remaining un-built pieces.

## Follow-up (same session) — editor UX + save/reload verification

Three asks after the initial ship, all addressed:

1. **Switched the template body editor back to JS** (reuse the shipped `@codemirror/lang-javascript`, not a
   new dep). Reverted `CodeEditor` to a single JS/JSX mode; dropped `lang="html"` from
   `TemplateSourceField`; removed `@codemirror/lang-html` from `package.json`. The body is HTML-with-
   `{{path}}`, but JS/JSX highlighting is the house pattern (`SqlEditor`/`PlotCodeField` use the same) and
   the user asked to reuse the existing package.

2. **The CodeMirror theme now tracks the shell's light/dark mode.** `editors/theme.ts` exports a
   `useCodeMirrorTheme()` hook that reads `useThemeOptional().theme.mode` and hands `@uiw/react-codemirror`
   `theme="dark"|"light"`; the chrome stays bound to the shell's CSS variables (`--fg`/`--muted`/`--accent`).
   The old `{ dark: true }` static marker is gone — it didn't drive the syntax colors (the built-in theme
   prop does), so in light mode the code was illegible. `CodeEditor` + `SqlEditor` both use the hook.
   Defaults to dark outside a `ThemeProvider` (tests / standalone mounts).

3. **The default template body is now a building-data example** bound to the seeded postgres datasource
   (`docker/postgres/seed.py`: site / meter / point / point_reading). The `DEFAULT_INLINE_CODE` renders a
   per-site summary (`{{#each rows}}{{site}} {{avg_val}}{{/each}}` + a `[data-call]` refresh); the helper
   text shows the `federation.query` SQL to bind over the seeded tables.

### Save/reload — verified correct (the reported "code not persisted/reloading")

The user reported the code wasn't persisted or reloaded after Save. Three new tests pin every link of the
chain, all green:

- `cellEditorState.test.ts` — `options.code` (Inline) + `options.templateId` (Saved) round-trip through
  `editorStateToCell(cellToEditorState(cell))`; a simulated editor flow (`defaultCell("template")` → patch
  `carry.extraOptions.code` → `editorStateToCell` → `cell.options.code` → reload into `extraOptions.code`)
  preserves the body end to end. The serializer is NOT the bug.
- `templateView.gateway.test.tsx` (real gateway) — `saveDashboard` with a template cell carrying
  `options.code`, then `getDashboard`, asserts the body survives the host round-trip AND the reloaded cell
  renders in-process (no iframe). Same for `options.templateId` (Saved mode resolves via `template.get`).
  The host is NOT the bug (options are opaque; `check_view_cells` validates view NAMES only).

So the save/reload logic is correct in the shipped code. If the symptom persists in the running app the
likely real-world causes are: (a) a stale dev server / hot-reload not picking up the new
`TemplateOptionsEditor` wiring — a clean rebuild clears it; (b) the dashboard *view* (separate from the
Data Studio preview) doesn't auto-refresh after a save — reload the dashboard; or (c) a fresh template
cell with no source bound shows the standard "no access to this source" panel (not the body) until a
source is picked in the Query tab — bind a `federation.query`/`store.query` source to see the body render.
