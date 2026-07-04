# Dashboard scope — the render-template widget, in-process (no iframe)

Status: **shipped** — 2026-07-05 (see
[`../../../sessions/frontend/dashboard/render-template-inprocess-session.md`](../../../sessions/frontend/dashboard/render-template-inprocess-session.md)).
Promotes the in-process render tier into [`../../../public/frontend/dashboard.md`](../../../public/frontend/dashboard.md)
(done). Topic: `frontend` (dashboard). Parent: the shipped
[render-template widget](render-template-widget.md) (now the reference for the `plot`/`d3` iframe tier,
which does **not** change), the [widget-builder scope](widget-builder-scope.md) ("Scripted views", the
trust-tier rule), and the [Data Studio editing UX](data-studio-ux-scope.md) (where the user edits it).

> **One host gap surfaced during build** (out of this scope's "no pipeline change" boundary, tracked in
> [`../../../debugging/frontend/rules-as-source-render-path-empty.md`](../../../debugging/frontend/rules-as-source-render-path-empty.md)):
> a view bound to `{tool:"rules.run"}` renders zero rows through `viz.query` for EVERY view (not just
> template) — the rules-as-source RENDER path was never driven against the real gateway. The in-process
> `TemplateView` is source-agnostic and correct (series/SQL renders real rows); it needs NO template-
> side change once that host gap is fixed. The gateway test's rules case is `it.skip` with a precise
> note.

> Naming note: the shipped iframe stack is described in [`render-template-widget.md`](render-template-widget.md)
> (kept as the reference for the `plot`/`d3` iframe tier, which does **not** change). This scope is the
> **new `template` render path**; it does not overwrite that doc.

## The ask

The `template` widget — an author-written HTML snippet that binds the panel's source rows via
`{{path}}`/`{{#each}}` and calls granted write tools from `[data-call]` buttons — currently renders
inside a **sandboxed opaque-origin iframe**. That iframe buys nothing for `template` (it runs **no
author JavaScript** — only pure interpolation + `innerHTML`) and costs a lot: a second document, a
per-tick `postMessage` data tax, no shell theme/fonts, a `connect-src 'none'` CSP that blocks live
watch niceties, and a jarring embedded-frame feel next to the in-process chart/stat/genui/ext tiles.
Make the `template` widget **render in-process** — a first-class dashboard view like `gauge` or
`genui` — while keeping its exact data contract and its leashed, host-re-checked write bridge.

Three concrete requirements from the ask:

1. **Works in the dashboard** — a `view:"template"` cell renders in-process through `WidgetView`,
   inheriting the shell's theme, the one `usePanelData` path, auto-refresh, and the deny/empty states.
2. **Editable in Data Studio** — the user authors/edits the template body in the panel editor (the
   [Data Studio UX](data-studio-ux-scope.md)) using the **already-shipped `CodeEditor` (CodeMirror)**,
   with a live in-process preview (no iframe rebuild flicker on each keystroke).
3. **Works with all sources, including rules** — the template binds rows from **any** picker source
   (Series / SQL / Live / Extension / Flows / **Rules**) with no per-source code, because it consumes
   the resolved frames from `usePanelData` exactly as every other data view does.

## Goals

- **`template` renders in-process**, not in an iframe — a new `TemplateView` sibling of `GenUiView`,
  routed by `WidgetView` for `view:"template"`. `plot`/`d3` **stay** on the iframe tier (they `eval`).
- **The data contract is unchanged.** Rows arrive through `usePanelData` (`{rows, latest, loading,
  denied}`); the same `interpolateTemplate` (pure, eval-free, already unit-tested) produces the markup;
  `[data-call]` buttons route through the same leashed `makeWidgetBridge` (local leash + host re-check;
  the token never enters this layer). A template cell authored today renders identically after.
- **Author markup is made safe for in-process `innerHTML`** — the one real delta the iframe used to
  cover. In-frame, injected markup was confined to an opaque origin; in-process it would run in the
  **shell document**. So the rendered HTML is **sanitized** (scripts, event-handler attributes,
  `javascript:` URLs, and `<iframe>`/`<object>`/`<embed>` stripped) before it touches the DOM — plus a
  defense-in-depth **authoring trust gate** (below). `interpolateTemplate` already HTML-escapes every
  *data* value; this adds the missing guard on the *author markup itself*.
- **Editable in Data Studio with a live preview** — wire template authoring into the panel editor. The
  editor components already exist but are **orphaned**: `builder/editors/TemplateSourceField.tsx` (the
  Inline↔Saved toggle) and `builder/editors/CodeEditor.tsx` (CodeMirror) are ported in but **no
  `panel-builder` tab imports them**, and `panel-builder/tabs/PanelOptionsTab.tsx` has **no `template`
  case** (it falls to "no per-viz options"). This scope **adds that seam**: a `template` branch in the
  options rail that mounts `TemplateSourceField`, so a user editing a `view:"template"` cell gets the
  code editor and a live in-process `TemplateView` preview (re-rendering on edit, no frame rebuild).
  Inline (`options.code`, ≤4 KB) and Saved (`options.templateId` → `render_template`, ≤64 KB) both work.
  No panel-kit serializer change: `code`/`templateId` are not `OWNED_OPTION_KEYS`, so they already
  round-trip verbatim through `cellEditorState`'s `carry.extraOptions`.
- **Rules (and every other source) work for free** — no rules-specific code. Confirmed: a Rules picker
  entry already produces `{tool:"rules.run", args:{rule_id, params}}` (shipped —
  [rules-as-source](rules-as-source-scope.md)), which resolves to `ctx.data` frames through
  `usePanelData` like any source; the template reads `{{#each rows}}` over them.

## Non-goals

- **`plot` and `d3` stay in the iframe.** They run author JS via `new Function` and fetch a CDN — moving
  them in-process is genuine RCE and is explicitly **out of scope** (a separate, harder decision). Only
  the **eval-free `template` engine** is promoted. `scriptedTier()` keeps returning `iframe` for those.
- **No change to the data pipeline.** `usePanelData`, `viz.query`, the frames-in contract, source
  resolution, watch/SSE — all untouched. This is a **render-tier** change for one view.
- **No new source or picker work.** Rules-as-source is already shipped; this scope *consumes* it. No
  branch on `rules` (or any tool id) anywhere — rule 10 holds (the template sees opaque rows).
- **No richer template language** (no `{{#if}}`, no nested `each`, no components). The interpolator is
  reused verbatim; extending it is a separate ask. (An author who needs components uses `genui`.)
- **No server-side rendering / no host code execution.** The host still only *stores* the template
  string (`render_template` record, unchanged); rendering is client-side, now in-process.
- **No RN/app renderer.** This is the web shell; the app is a later task (it would reuse the same
  in-process posture, not the iframe).

## Intent / approach

**Do for `template` what `genui` already did (`GenUiView`, "originally sandboxed iframe → amended to
in-process"): render trusted, data-bound output with our own code, in-process, over `usePanelData`,
with the leashed bridge for writes.** The template differs from genui in exactly one way — genui renders
a **catalog IR through our React components** (no author strings reach the DOM), whereas a template
renders **author-written HTML markup** via `innerHTML`. That single difference is the whole security
design of this scope, and it is solved by **sanitizing the markup** (+ a trust gate), not by a sandbox.

### The stack (mirrors `GenUiView`, one responsibility per file — FILE-LAYOUT)

- **`views/TemplateView.tsx`** (new) — the in-process view. `usePanelData(cell, scope, refreshKey)` →
  `interpolateTemplate(code, {rows, latest, loading, denied})` → **`sanitizeTemplateHtml(...)`** → a
  `<div dangerouslySetInnerHTML>` inside the shell's widget chrome. After commit, it queries the
  container for `[data-call]` elements and wires their click → `bridge.call(tool, args)` (leash-checked),
  stamping `data-called="ok"|"err"` — the same behavior the in-frame runtime had (~18 lines ported from
  `iframeRuntime.ts`'s `template` branch). Inline vs Saved code resolution is lifted from `ScriptedView`
  (`options.code` else `getTemplate(templateId)`).
- **`builder/sanitizeTemplateHtml.ts`** (new) — a pure `string → string` sanitizer that wraps
  **DOMPurify** (Decision 1) behind one module with our config: allow a conservative structural tag/
  attribute set + `data-call`/`data-args` (`ADD_ATTR`); forbid `<script>/<iframe>/<object>/<embed>/
  <link>/<meta>/<base>`, all `on*` handlers, `javascript:`/non-image `data:` URLs, and `style`
  expressions. Pure (deterministic for a given input) and unit-tested directly against the XSS-vector
  suite. DOMPurify is the **one new npm dependency** this scope adds — pinned, on the browser-only render
  path, wrapped in this single named file so the seam stays swappable while the parse/strip stands on
  audited code (see Decision 1 for why not hand-rolled).
- **`WidgetView.tsx`** — route `case "template"` → `<TemplateView>` (was `<ScriptedView engine="template">`).
  `plot`/`d3` cases unchanged (still `ScriptedView` → iframe).
- **`panel-builder/tabs/PanelOptionsTab.tsx`** (+ the options-rail wiring) — add the missing `template`
  branch that mounts `TemplateSourceField` (today the view has **no** authoring surface in the builder;
  the editor components are orphaned). This is the "editable in Data Studio" deliverable — one wiring
  point, not new editor code.
- **`trust.ts`** — unchanged in signature; a doc note that `template` is no longer a `scriptedTier`
  consumer. `scriptedTier()` still governs `plot`/`d3` only. (No new tier enum value needed — `template`
  simply stops asking for a tier and renders like `genui`.)

### The authoring trust gate (defense in depth)

Sanitization is the load-bearing guard, but we also make **authoring a `template` cell a capability**,
mirroring genui's "the `dashboard.save` cap is the trust gate" and the ext-widget "install is the trust
gate" posture. Two layers:

1. **Sanitize always** (every render, every viewer) — the hard wall. A malicious stored template can
   never execute script in the shell, regardless of who authored it.
2. **Gate authoring** — writing a cell whose `view:"template"` carries inline `options.code` (or a
   `render_template` body) rides the existing `mcp:dashboard.save:call` / `mcp:template.save:call`
   grants; a viewer only *renders* (sanitized). This means the population that can introduce template
   markup is already the population trusted to author dashboards — the same trust class as genui.

Rejected: **keep the iframe.** It works, but it is the wrong tier for eval-free data-binding — it
imposes the double-document/postMessage tax and the broken theme/feel the ask is about, to guard against
a JS-execution risk `template` doesn't have. genui already set the precedent that trusted, data-bound,
non-eval output belongs in-process. Rejected: **a client-side transform/render mirror** — N/A, we reuse
the one `interpolateTemplate`. Rejected: **a hand-rolled sanitizer** — we adopt DOMPurify instead
(Decision 1): an XSS boundary should stand on audited code, not a bespoke allow-list we maintain.

## How it fits the core

- **Tenancy / isolation (rule 6):** unchanged. The template resolves rows through `usePanelData` →
  `viz.query`/watch, each workspace-first at the host; the cell is a workspace-scoped `dashboard`/
  `render_template` record. No cross-workspace reach is introduced (nothing new resolves).
- **Capabilities (rule 5/7):** **no new capability.** Rendering is a client concern over already-gated
  reads (`mcp:viz.query:call`, the source's own cap — e.g. `mcp:rules.run:call`, re-checked per call);
  authoring rides `mcp:dashboard.save:call` / `mcp:template.save:call` (unchanged); a `[data-call]`
  write is leashed to `cell.tools ∩ grant` and **re-checked at the host per call** (the token never
  enters the view). Deny path: a source the viewer lacks renders the standard `usePanelData` denied
  panel; a `data-call` to a tool outside the leash is rejected locally and again server-side.
- **Placement:** client-only render change; symmetric (no `if cloud`). N/A for node placement.
- **MCP surface:** **none new.** `template.*` CRUD (get/list/save/delete) is unchanged; `viz.query` and
  the source verbs are unchanged. No get/list/watch/batch added — this is a render-tier change, not an
  API. State: the template string stays a `render_template` record (SurrealDB, rule 2) — the host still
  **never executes** it.
- **State vs motion / bus / secrets / SDK-WIT:** N/A — no persistence, motion, secret, or plugin-
  boundary change. The `render_template` model's doc comments ("rendered in the sandboxed iframe tier")
  need a **text update** to say "the `template` engine renders in-process; `plot`/`d3` in the iframe".
- **One responsibility per file (rule 8):** `TemplateView.tsx`, `sanitizeTemplateHtml.ts` (+ its test),
  and the `data-call` wiring each land as their own file; `ScriptedView.tsx` keeps only `plot`/`d3`.
- **Dependencies:** two new **client-only** npm deps — **`dompurify`** (the sanitizer, Decision 1) and
  **`@codemirror/lang-html`** (the editor mode, Decision 3), both pinned. No Rust/crate, no WIT, no
  workspace-root lockfile churn beyond `ui/`. Flagged here because a scope that adds a security-boundary
  dependency must say so loudly.
- **Skill doc:** **N/A** as a new skill — this is a UI render change, not a new agent-drivable surface.
  The template *authoring contract* is already an agent surface via the shipped
  [dashboard-widgets skill](../../../skills/dashboard-widgets/SKILL.md) and the render-envelope in
  [render-template-widget.md](render-template-widget.md); the implementing session **updates** those
  (and the public doc) to note `template` renders in-process (the `data`/`source`/`data-call` contract
  the agent emits is otherwise unchanged).

## Example flow

1. In Data Studio, the user picks a **Rules** source — *Hourly mean* — from the source combobox; the
   picker sets the cell target `{tool:"rules.run", args:{rule_id:"r1", params:{…}}}`. Auto-run fires;
   the status bar shows rows resolved.
2. They switch the view to **Template** and edit the body in the CodeMirror `CodeEditor`:
   `<ul>{{#each rows}}<li>{{hour}}: {{mean}}</li>{{/each}}</ul>`. The **in-process** preview
   (`TemplateView`) re-renders on each keystroke against the frames already fetched — no iframe rebuild,
   shell fonts/theme, instant.
3. They add a write button: `<button data-call="rules.run" data-args='{"rule_id":"r1"}'>Recompute</button>`
   and add `rules.run` to the cell's tools. Clicking it routes through the leashed bridge; the host
   re-checks `mcp:rules.run:call` + workspace.
4. They **Save to tab**, then **Save as library panel** — persisting the cell (inline `options.code`) or
   a shared `render_template`. The dashboard renders the tile in-process alongside the gauges.
5. A hostile author saves `<img src=x onerror="fetch('/steal')">`. On render, `sanitizeTemplateHtml`
   strips the `onerror` handler; the image simply fails to load. **No script runs in the shell.**
6. Contrast (unchanged): a `plot` cell still mounts in the sandboxed iframe — its author JS is never
   run in-process.

## Testing plan

Per [`../../testing/testing-scope.md`](../../testing/testing-scope.md) — real gateway, real store,
no fakes (rule 9). Mandatory categories + the render-tier specifics:

- **Sanitizer unit (pure, direct — the security core):** `sanitizeTemplateHtml` strips `<script>`,
  `on*=` handlers, `javascript:`/non-image `data:` URLs, `<iframe>/<object>/<embed>`, and
  `style:expression(...)`; **keeps** allowed structural markup + `data-call`/`data-args`; is idempotent;
  never throws on malformed input. Property-style cases for each XSS vector. This test is the wall — it
  must be exhaustive, because it replaces the sandbox.
- **Interpolation regression:** `interpolateTemplate` behavior is unchanged (its existing test stays
  green); the in-process view produces the same escaped output the frame did for the same rows.
- **In-process render (component/unit):** `TemplateView` renders rows from a seeded `usePanelData`
  state; `[data-call]` click calls the leashed bridge; a `data-call` **outside** the cell's tools is
  rejected (leash) — assert no `invoke` for the out-of-leash tool. Denied source → the standard denied
  panel (parity with other views). No iframe element is mounted (assert `template` no longer renders
  `WidgetIframe`).
- **Any-source incl. rules (gateway, `pnpm test:gateway`, mandatory no-mock path):** seed a saved rule
  returning records into the real store; drive a `template` cell bound to `{tool:"rules.run"}` over a
  spawned gateway; assert the template renders the real rows in-process. Repeat the parity for a
  series/SQL source (one source-agnostic path).
- **Capability deny (mandatory):** a `data-call` write with no grant is denied at the host (guard 3
  survives the tier change); a source read the viewer lacks renders denied, not a crash.
- **Workspace isolation (mandatory):** a `render_template`/template cell in ws-A is invisible to ws-B
  (the `template.*` verbs are already ws-first; assert the render path introduces no leak).
- **Data Studio editing (component/gateway):** editing the body in `CodeEditor` re-renders the
  in-process preview without a frame rebuild and without re-fetching (the data-studio fetch/shape split
  already covers this — assert an edit-the-code loop triggers no `viz.query`); Inline↔Saved both resolve.
- **Regression:** `plot`/`d3` still mount the iframe (their tier is untouched); a channel `rich_result`
  with `view:"template"` renders in-process through the same `WidgetView` (the response surface inherits
  the tier change with no channel-specific code).
- **Belt-and-braces (Decision 5):** the `data-call` wiring reads only `data-*` attributes (a test asserts
  an author inline handler is never read/executed), and — if the shell CSP supports it — a Trusted-Types
  smoke check that the sanitizer output is the only script sink. Even a stubbed "sanitizer returns unsafe
  HTML" must not produce an inline-script execution in the mount.

## Risks & hard problems

- **The sanitizer IS the security boundary now.** With the iframe gone for `template`, a gap in
  `sanitizeTemplateHtml` is an XSS in the shell (cookies, token-adjacent). This is the single most
  underestimated part. Mitigations (all committed, see Decisions): (1) **DOMPurify** does the parse/strip
  — audited, deny-by-default config, not a bespoke walker (Decision 1); (2) an exhaustive XSS-vector test
  suite is the definition of done (mutation-XSS, `svg`/`math` namespace tricks, `data:`/`javascript:`
  URLs, malformed/round-trip input); (3) the authoring cap-gate keeps the input population trusted
  (Decision 2); (4) a render-time Trusted-Types/CSP posture + `data-*`-only wiring so a hypothetical miss
  has no inline-script sink (Decision 5); (5) the sanitizer is **one file** so a config/library change is
  contained. Do **not** ship the in-process path without the full XSS suite green.
- **`dangerouslySetInnerHTML` + event wiring after commit.** React owns the subtree; the `[data-call]`
  listeners are attached imperatively post-commit (like the frame did) and must be re-attached on every
  re-render and cleaned up on unmount (no listener leak, no double-fire). Mirror `GenUiView`'s
  post-commit effect discipline.
- **Feature parity with the frame's live path.** The iframe supported `bridge.watch` (series/bus SSE)
  from inside a widget. In-process, `usePanelData` already handles watch sources for the *data*; a
  template that used `data-call` to a watch verb is rarer — confirm the leashed bridge's `watch` still
  works in-process (it does; `makeWidgetBridge` is transport-only) and tear down on unmount.
- **CodeMirror HTML mode.** `@codemirror/lang-html` is a new dependency (only `lang-javascript`/`lang-sql`
  are present); Decision 3 adds it. Additive and low-risk — the `CodeEditor` already takes a language
  extension; this is a prop swap for the `template` engine.
- **Doc drift.** [`render-template-widget.md`](render-template-widget.md) and the `render_template`
  model comments describe "iframe" as the tier for all three engines. The session must update them to
  split `template` (in-process) from `plot`/`d3` (iframe), or the docs mislead the next author.

## Decisions (resolved — build to these)

These were open questions; they are now settled so the session builds without re-litigating. Each names
the alternative rejected and why (house style).

1. **Sanitizer = adopt a vetted library (DOMPurify), not hand-rolled.** *Decided: use DOMPurify.* This
   reverses the earlier "hand-rolled allow-list" lean after weighing it long-term: a hand-written HTML
   sanitizer is precisely where XSS hides (mutation-XSS, namespace confusion, parser round-trip quirks) —
   the exact class of bug that lives in the tail of a bespoke allow-list and that no test suite fully
   closes. DOMPurify is the industry-standard, audited, actively-maintained answer; pinning it and
   wrapping it in **one file** (`sanitizeTemplateHtml.ts`) with **our** config (allow the structural tag/
   attribute set + `data-call`/`data-args`; forbid `<script>/<iframe>/<object>/<embed>/<link>/<meta>/
   <base>`, all `on*`, `javascript:`/non-image `data:` URLs, `style` expressions) keeps the seam swappable
   while standing on reviewed code for the actual parse/strip. This is a **new npm dependency** — the one
   this scope adds — justified because it sits on the security boundary that replaces the sandbox. *(This
   is not a rule-9 "fake": it is a real, vetted external library on a browser-only path, wrapped behind
   one named module — the sanctioned pattern.)* Rejected: hand-rolled walker — smaller diff, zero deps,
   but it makes us the maintainers of a security-critical parser we cannot fully audit; not worth it for
   an XSS boundary. The **belt-and-braces** stays regardless (Decision 5).
2. **Authoring cap = reuse `dashboard.save` / `template.save`.** *Decided: no new cap.* Introducing
   template markup rides the existing dashboard/template write grants, matching genui's "the save cap is
   the trust gate." Rejected: a distinct `template:author` cap — it would split the "may build dashboards"
   population for a distinction no caller needs yet; additive later if a workspace ever wants "dashboards
   yes, raw HTML no."
3. **Add `@codemirror/lang-html` = yes.** *Decided: add it.* Small, and the body is HTML/JSX — syntax
   highlighting + bracket matching is the whole point of "editable in Data Studio." Rejected: plain-text
   fallback — acceptable but a worse loop than the ask deserves.
4. **Retire the in-frame `template` branch in `iframeRuntime.ts` = yes, same session.** *Decided: remove
   it.* Once `TemplateView` ships, the frame's `template` engine is dead code and a second place
   `interpolateTemplate` is embedded (via `.toString()`). Removing it keeps `interpolateTemplate` a single
   in-process caller and shrinks the sandbox to what still needs it (`plot`/`d3`). Rejected: leave it
   inert one release for rollback — the change is behind `WidgetView`'s `case "template"`; a rollback is a
   one-line route revert, so dead-code-for-rollback earns nothing.
5. **Belt-and-braces: a render-time CSP guard on the mount, independent of the sanitizer.** *Decided:
   add it.* Beyond DOMPurify, the `TemplateView` mount is rendered under a **`trusted-types` / CSP posture
   as tight as the shell allows** and the `data-call` wiring reads only `data-*` attributes (never
   author-supplied inline handlers), so even a hypothetical sanitizer miss has no inline-script sink. If
   the shell's CSP can carry a `require-trusted-types-for 'script'` directive, the sanitizer output is the
   only Trusted Types sink — a defense-in-depth that turns "sanitizer bug" from RCE into a blocked write.
   Session confirms how far the shell's existing CSP can go; the sanitizer + `data-*`-only wiring is the
   floor regardless.

## Open questions (resolved during build — see the session doc)

1. **Exact DOMPurify allow-list for `data-call`/`data-args`.** *Resolved (test-first):* the XSS suite
   (`sanitizeTemplateHtml.test.ts`, 16 cases) drove it. `ADD_ATTR`-equivalent: `data-call` + `data-args`
   explicitly in `ALLOWED_ATTR`; `ALLOW_DATA_ATTR: true` admits inert `data-*` for CSS selectors; a
   conservative structural tag/attribute set; every `on*` in an explicit `FORBID_ATTR`; a `style`-
   scrubbing `afterSanitizeAttributes` hook for `expression()`/`-moz-binding`/`behavior` (jsdom's CSS
   parser doesn't reject them, so the hook is load-bearing for the suite). The suite IS the definition
   of done.
2. **Does the shell CSP already forbid inline script (so Trusted Types is reachable)?** *Resolved:*
   the shell has **NO CSP** today (no `Content-Security-Policy` meta in `index.html`). A
   `require-trusted-types-for 'script'` is document-wide, so it cannot be added narrowly to the
   `TemplateView` mount without a shell-wide CSP scope (the shell's own inline bootstrap `<script>`
   would violate a strict `script-src`). Decision 5's Trusted-Types ceiling is therefore **deferred**
   — its own shell-wide CSP scope; the floor (sanitizer + `data-*`-only wiring + the XSS gate) shipped.

## Related

- [`render-template-widget.md`](render-template-widget.md) — the shipped iframe stack (reference for the
  `plot`/`d3` tier that does **not** change; the `template` half is superseded by this scope).
- [`data-studio-ux-scope.md`](data-studio-ux-scope.md) — the editor surface where the template is authored
  (the `CodeEditor`, the fetch/shape split that makes edit-without-requery free).
- [`rules-as-source-scope.md`](rules-as-source-scope.md) — the Rules picker source this consumes
  (`rules.run {rule_id, params}` → records → `ctx.data`), the reason "works with rules" needs no new
  code. **Note (for the session): that doc's status line still reads "scope (the ask)" but the feature
  is SHIPPED** end-to-end (`rulesEntries`/`listRules` in the package + `useSourcePicker.ts` +
  `rulesSource.gateway.test.tsx`) — flip it to shipped when this lands, or separately.
- [`source-picker-package-scope.md`](source-picker-package-scope.md) — the `@nube/source-picker` that
  produces every `{tool,args}` source the template binds.
- [`widget-builder-scope.md`](widget-builder-scope.md) — "Scripted views" + the trust-tier rule this
  scope amends for `template` (the genui precedent for in-process trusted rendering).
- `ui/src/features/dashboard/views/genui/GenUiView.tsx` — the in-process, `usePanelData`-driven,
  leashed-bridge precedent `TemplateView` mirrors.
- `ui/src/features/dashboard/builder/templateInterpolate.ts` — the pure interpolator reused verbatim.
- Core rules: README §3 (rules 2/4/5/6/7/10); the debugging note
  `docs/debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md` (why the iframe tier is
  reserved for untrusted author code).
- Promotion target: [`../../../public/frontend/dashboard.md`](../../../public/frontend/dashboard.md).
- Skill to update (not create): [`../../../skills/dashboard-widgets/SKILL.md`](../../../skills/dashboard-widgets/SKILL.md).
```
