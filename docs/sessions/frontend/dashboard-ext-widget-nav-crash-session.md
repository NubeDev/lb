# Dashboard ext-widget teardown wedges navigation — session

**Date:** 2026-07-03
**Area:** frontend / dashboard + ext federation
**Ask:** "New issue: when I click on a dashboard page with this widget the navigation breaks"
(the thecrew `ext:thecrew/scene` graphics-canvas widget — `rust/extensions/thecrew`).

## Diagnosis — first guess wrong, Playwright got the truth

My first fix (guarding the cleanup against the tile *throwing* on WebGL disposal + an `ExtErrorBoundary`)
**did not fix it** — the user reported the same issue. I then drove the real dev shell with Playwright
(`:5173`, `#/…&d=scene-dash` → click "Rules") and captured the actual errors:

```
Warning: Attempted to synchronously unmount a root while React was already rendering …
PAGEERROR: Failed to execute 'removeChild' on 'Node': The node to be removed is not a child of this node.
  at removeChildFromContainer … at commitDeletionEffectsOnFiber …
```

Real root cause: the tile owns its OWN React root (`createRoot(el)`); `ExtWidget`'s effect-cleanup called
that root's `unmount()` **synchronously**. Cleanup runs during the SHELL's own commit (a route change
unmounts the cell), so a nested synchronous `root.unmount()` made React double-remove the same DOM →
`removeChild: not a child` thrown **inside React's commit** (uncatchable by our `try/catch`) → shell
commit aborted → nav wedged. No extension throw involved — the synchronous nested unmount alone did it.

## Fix (host-side, protects every in-process ext mount)

**Defer the tile unmount to `queueMicrotask`** (runs after the shell commit) and stop hand-clearing the
container the tile root owns (a `replaceChildren()` double-removes → same `NotFoundError`):

1. [ExtWidget.tsx](../../../ui/src/features/dashboard/builder/ExtWidget.tsx) — dashboard cell (the bug).
2. [ExtHost.tsx](../../../ui/src/features/ext-host/ExtHost.tsx) — extension page (latent identical bug;
   fixed proactively).
3. [WidgetView.tsx](../../../ui/src/features/dashboard/views/WidgetView.tsx) — keeps `<ExtWidget>` in
   `ExtErrorBoundary` as a belt-and-braces render-throw wall (NOT what fixed this bug).

No thecrew change, no backend change.

## Verification (real browser)

Playwright repro, after the fix: scene dashboard loads (`data-ext-widget` cell + `canvas` both present →
widget really mounted), click "Rules" → URL becomes `#/t/nube/rules`, sidebar still present, and the only
console line is a clean `THREE.WebGLRenderer: Context Lost` (the now-deferred GL teardown). No
`synchronously unmount` / `removeChild` / pageerror.

## Tests

- [ExtWidget.test.tsx](../../../ui/src/features/dashboard/builder/ExtWidget.test.tsx) — regression
  rewritten to pin the actual mechanism: "defers the tile unmount out of the synchronous cleanup" asserts
  the tile `unmount` is NOT called synchronously during React's unmount commit, and the deferred microtask
  still runs it once. **3/3 green.** jsdom can't reproduce the WebGL/commit race; the browser proof is the
  Playwright repro above.
- Full unit suite: the 17 pre-existing failures (theme/agent/channel/app — unrelated files) are
  unchanged; no new regressions.

## Debugging history

Logged [frontend/dashboard-ext-widget-teardown-wedges-nav.md](../../debugging/frontend/dashboard-ext-widget-teardown-wedges-nav.md)
+ a README row (both corrected from the wrong first-guess cause to the synchronous-nested-unmount cause).

## Follow-up worth considering

A committed Playwright e2e (mount thecrew scene cell → navigate away → assert URL changed + sidebar
present, no pageerror) would guard this in CI where the built-shell gateway suite runs; I proved it with a
throwaway spec this session but did not commit an e2e.
