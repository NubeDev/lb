# Session — template widget: real data binding for dashboard **and** channels

## Ask

The `template` scripted view (the sandboxed-iframe render widget) was to be usable in
**channel rich responses**, not just dashboards, and to actually render its source rows.
The user's steer: "do whatever is best long term, needs to be used for both."

## What was already true (no move needed)

Channels already reuse the dashboard render stack: `ResponseView → WidgetView`, and
`WidgetView` has `case "template" → ScriptedView → WidgetIframe`. So a `view:"template"`
response *mounts* — but the engine had **no data path**: it set `innerHTML = code` verbatim
and the advertised `{{path}}` interpolation was never implemented. That was the real gap.

## What shipped

The root fix was to give scripted views the **same data path every read view has** and a
real, eval-free interpolator — one change that serves both surfaces because both build a
`Cell` and flow through `usePanelData`.

1. **New `templateInterpolate.ts`** — a pure, closure-free `interpolateTemplate(code, data)`:
   `{{path}}` + single-level `{{#each list}}…{{/each}}`, **data values HTML-escaped** (markup
   is author-trusted, data is the viewer's grant). Unit-tested directly (9 tests).
2. **One source of truth** — the frame runs the *same* function, embedded via
   `interpolateTemplate.toString()` in `buildIframeSrcdoc` (it's closure-free, so the
   serialized form is complete).
3. **Real rows via the one hook** — `ScriptedView` now takes the `cell` + `scope` +
   `refreshKey`, calls `usePanelData(cell, …)`, and passes `{rows, latest, loading, denied}`
   to the frame. Identical on a dashboard and in a channel (both are source-backed `Cell`s).
4. **Live updates without rebuild** — rows go in as the initial `srcdoc` config AND as
   `render-data` postMessages; a `frame-ready` handshake covers the load race. The iframe is
   never rebuilt on a data change (no CDN re-fetch for plot/d3). `plot`/`d3` also get `rows`
   as a 4th arg.
5. **Builder** — `DEFAULT_INLINE_CODE` now shows `{{#each rows}}`; the helper text documents
   the binding syntax.

Files: `templateInterpolate.ts` (+test), `iframeRuntime.ts`, `WidgetIframe.tsx`,
`ScriptedView.tsx`, `WidgetView.tsx`, `editors/TemplateSourceField.tsx`.
Doc: `docs/scope/frontend/dashboard/render-template-widget.md` (incl. the channel envelope).

## Tests

- `templateInterpolate.test.ts` — 9 passing (binding, each, escaping, object→JSON, unknown).
- Full UI unit suite green: **48 files / 322 tests**. `tsc --noEmit` clean (only the
  pre-existing unrelated `FlowsCanvas.gateway.test` errors remain).

## Gotcha logged

Embedding a `//` comment that contained literal backticks inside the `srcdoc` **template
literal** silently closed the outer string → `TS2349 "Type 'String' has no call signatures"`.
See `docs/debugging/frontend/srcdoc-template-literal-backtick-break.md`.

## Follow-ups (not done)

- Nested `{{#each}}` and `{{#if}}` conditionals (single-level each only today).
- A real-gateway render test that seeds rows and asserts the iframe `srcdoc` embeds them
  (jsdom does not execute `srcdoc` scripts, so the *interpolation* is covered by the pure
  unit test; the *data wiring* could be asserted via the emitted `srcdoc` payload).
