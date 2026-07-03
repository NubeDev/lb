# HANDOVER — Dashboard with thecrew scene widget breaks navigation

**Status:** 🟡 ROOT CAUSE FOUND + FIXED IN TREE, awaiting user re-test. Console tracing added to
`ExtWidget` (round 3) captured the true sequence in the user's dev browser: the effect double-runs
(StrictMode) with `cleanup SCHEDULED {hasTeardown: false}` — i.e. cleanup fires before the async
`mount()` resolves, so the tile's React root is created LATER and orphaned, and successive runs sharing
one container wipe each other's live-root DOM → `removeChild: not a child` inside the shell commit → nav
wedges. The two earlier fixes (try/catch + ErrorBoundary; microtask defer) treated symptoms, not the
orphan. Fix: give each effect-run its own private child node + keep its teardown in-closure (StrictMode-
safe async mount). See `docs/debugging/frontend/dashboard-ext-widget-teardown-wedges-nav.md` for the full
corrected write-up. **User still needs to confirm in their browser** (§5.1/§5.2 below).

**Copy/paste this whole file into a fresh issue / new session.**

---

## 1. Symptom (as reported by the user)

- Open a dashboard whose grid contains thecrew's graphics-canvas widget (`ext:thecrew/scene`).
  The scene renders fine (AHU-1 duct diagram, live values).
- After that, **navigation is broken** — clicking another dashboard in the roster, or another sidebar
  item (Rules, Flows, etc.), does nothing / the shell is wedged.
- Reproduces at: `http://localhost:5173/#/t/acme/dashboards?from=2026-06-03&to=2026-07-03&d=scene-dash`
  (dev server, workspace `acme`, dashboard id `scene-dash`).
- User has said "same issue" and "100% NOT fixed" after both attempted fixes below.

Screenshot the user provided shows the scene mounted and rendering correctly before nav breaks.

---

## 2. What I changed (BOTH attempts are currently in the working tree, uncommitted)

Files touched for THIS bug (there are other unrelated dashboard changes in the same worktree — see §7):

- `ui/src/features/dashboard/builder/ExtWidget.tsx` — the dashboard-cell federation mount.
- `ui/src/features/ext-host/ExtHost.tsx` — the extension-page federation mount (changed proactively).
- `ui/src/features/dashboard/views/WidgetView.tsx` — wrapped `<ExtWidget>` in `ExtErrorBoundary`.
- `ui/src/features/dashboard/builder/ExtWidget.test.tsx` — regression test (currently green).

### Attempt 1 (WRONG — hypothesis was "the tile throws on WebGL disposal")
Wrapped the tile `unmount()` in try/catch + added `ExtErrorBoundary` around `<ExtWidget>`. Did nothing,
because the real error is thrown *inside React's own commit phase*, not in our callback.

### Attempt 2 (current tree — "synchronous nested unmount during shell commit")
Deferred the tile `unmount()` to `queueMicrotask` and stopped manually calling `el.replaceChildren()`,
in BOTH `ExtWidget` and `ExtHost`. Current diff of the core file:

```ts
// ui/src/features/dashboard/builder/ExtWidget.tsx — effect cleanup
return () => {
  cancelled = true;
  const teardown = unmount;
  const ext = row.ext;
  queueMicrotask(() => {
    try {
      if (typeof teardown === "function") teardown();
    } catch (e) {
      console.error(`[ExtWidget] ${ext} tile failed to unmount cleanly`, e);
    }
  });
};
```

`WidgetView.tsx` also now renders:
```tsx
<ExtErrorBoundary ext={view.slice("ext:".length)} resetKey={`${view}:${cell.i}`}>
  <ExtWidget … />
</ExtErrorBoundary>
```

**The user reports the bug persists with attempt 2 in place.** So either the fix is incomplete, the
dev server didn't pick up the edit (HMR/stale state), or the failing interaction is different from what
I tested.

---

## 3. The ONE real data point (captured via Playwright, first run, BEFORE attempt 2)

Driving the real dev shell to the URL above and clicking "Rules" produced (this is genuine, not guessed):

```
[error] Warning: Attempted to synchronously unmount a root while React was already rendering.
        React cannot finish unmounting the root until the current render has completed …
    at ExtWidget (…/dashboard/builder/ExtWidget.tsx)
    at ExtErrorBoundary (…/ext-host/ExtErrorBoundary.tsx)
    at WidgetView (…/dashboard/views/WidgetView.tsx)
    at Grid (…/dashboard/Grid.tsx)
    at DashboardView (…/dashboard/DashboardView.tsx)

[PAGEERROR] Failed to execute 'removeChild' on 'Node': The node to be removed is not a child of this node.
    at removeChildFromContainer (react-dom)
    at commitDeletionEffectsOnFiber (react-dom)
    …
[log] THREE.WebGLRenderer: Context Lost.
```

This is the same secondary-error signature documented in
`docs/debugging/frontend/ce-page-crashes-openstream-detached-this-drops-shell.md`
("synchronously unmount a root while rendering" / "removeChild: not a child").

Interpretation: the tile owns its OWN React root (`createRoot(el)` in
`rust/extensions/thecrew/ui/src/mount.tsx` → `mountWidget`). Something is calling `root.unmount()`
synchronously while the SHELL is mid-commit, so React double-removes shared DOM → uncaught error inside
the commit → shell render aborts → nav wedges.

---

## 4. ⚠️ Why this is NOT confirmed fixed

After attempt 2, **my Playwright repro stopped showing the error** — URL changes to `#/t/acme/rules`,
sidebar survives, only a clean `THREE.WebGLRenderer: Context Lost` remains. I tested:
- Direct-load the scene dashboard → click sidebar "Rules": PASSES (no error).
- Select scene dashboard from roster → switch to another dashboard row: PASSES (no error).
- Confirmed the widget really mounts first (`[data-ext-widget]` count = 1, `<canvas>` count = 1).

**But the user says it still breaks.** So my repro is not exercising the user's actual failing path.
DO NOT trust "my Playwright is green" as proof — it wasn't for the user. The gap between my green repro
and the user's broken browser is the #1 thing to close.

---

## 5. Concrete next steps (in priority order)

1. **Get the user's real console output.** Ask them to open DevTools → Console, reproduce, and paste the
   FULL red error + stack. This is the fastest path — my synthetic repro diverges from theirs.
2. **Confirm the dev server actually loaded the edit.** Vite HMR can keep a stale module + a live WebGL
   root. Have the user do a HARD reload (Cmd/Ctrl-Shift-R) or restart `pnpm dev` after pulling, then
   retry. It's plausible attempt 2 was never actually running for them.
3. **Reproduce the EXACT user interaction.** My repro clicks sidebar "Rules". The user's break may be:
   - selecting a DIFFERENT dashboard from the roster while the scene one is open (roster re-render, not a
     route change) — I tested this and it passed, but try more rows / rapid clicks;
   - the browser back/forward button;
   - a hard page reload while already ON `d=scene-dash`;
   - a dashboard OTHER than `scene-dash` (e.g. `scene-build`, `scene-e2e` — all exist in the roster).
4. **If the sync-unmount theory holds but the microtask defer is insufficient:** the microtask may still
   run inside the same task/commit under some schedulers. Try instead:
   - unmounting via a `useLayoutEffect`-ordered teardown, or
   - `setTimeout(teardown, 0)` (macrotask — definitively after commit), or
   - NOT calling `root.unmount()` at all on the shell side and instead having the extension's
     `mountWidget` return a teardown that schedules its own unmount, or
   - keying the cell so React unmounts the whole `<div ref=elRef>` and letting the extension root be
     GC'd (verify no leak) rather than imperatively unmounting.
5. **Check for a SECOND mount/unmount source.** `ExtWidget`'s effect deps are
   `[row?.ext, tile?.entry, workspace, scopeKey, configKey]`. `scopeKey = JSON.stringify(scope)` and
   `configKey` change on every var/range tick — the URL has `from`/`to`, and there's auto-refresh. If
   `scope` identity churns, the effect re-runs (unmount+remount the tile) REPEATEDLY, possibly mid-render.
   **Suspect: the tile is being torn down and re-mounted on a range/var/refresh change, colliding with a
   navigation.** Log every mount/unmount with a timestamp and watch for a teardown firing during a shell
   render. This is my leading untested hypothesis.
6. **WebGL context exhaustion angle.** Multiple scene cells (or repeated remount from #5) can exhaust the
   browser's WebGL context pool; `THREE Context Lost` is already showing. A lost context mid-render could
   throw somewhere unguarded. Worth ruling out by testing with exactly one scene cell and no auto-refresh.

---

## 6. Repro harness (throwaway spec — recreate to iterate)

I used a temporary Playwright spec (deleted). Recreate at `ui/e2e/zzz-repro.spec.ts` and run with
`npx playwright test zzz-repro --reporter=line` (dev server must be up on :5173; login is
`user:ada` / `acme`). Skeleton:

```ts
import { test, type ConsoleMessage } from "@playwright/test";
const SHELL = "http://localhost:5173";
test("repro", async ({ page }) => {
  const logs: string[] = [];
  page.on("console", (m: ConsoleMessage) => logs.push(`[${m.type()}] ${m.text()}`));
  page.on("pageerror", (e) => logs.push(`[PAGEERROR] ${e.message}`));
  await page.goto(SHELL + "/", { waitUntil: "networkidle" });
  await page.getByLabel("identity").fill("user:ada");
  await page.getByLabel("workspace").fill("acme");
  await page.getByLabel("sign in").click();
  await page.waitForTimeout(1500);
  await page.goto(SHELL + "/#/t/acme/dashboards?from=2026-06-03&to=2026-07-03&d=scene-dash", { waitUntil: "networkidle" });
  await page.locator('[data-ext-widget]').first().waitFor({ state: "attached", timeout: 20000 }).catch(() => {});
  await page.waitForTimeout(4000);
  // …then reproduce the user's exact interaction and dump `logs`.
});
```
`playwright.config.ts` has `testDir: "./e2e"`, so the spec must live under `ui/e2e/`. Delete it when done
(don't commit a `zzz-` spec).

---

## 7. Relevant code map

| What | Where |
|---|---|
| Dashboard-cell mount (the bug site) | `ui/src/features/dashboard/builder/ExtWidget.tsx` |
| Cell dispatcher (wraps ExtWidget) | `ui/src/features/dashboard/views/WidgetView.tsx` |
| Grid that renders cells | `ui/src/features/dashboard/Grid.tsx` → `WidgetHost` |
| Ext page mount (twin path) | `ui/src/features/ext-host/ExtHost.tsx` |
| Crash boundary | `ui/src/features/ext-host/ExtErrorBoundary.tsx` |
| Federation loader | `ui/src/features/dashboard/builder/federationWidget.ts` |
| Extension mount (owns its React root) | `rust/extensions/thecrew/ui/src/mount.tsx` (`mountWidget`) |
| The scene widget | `rust/extensions/thecrew/ui/src/bridge/SceneWidget.tsx` |
| The WebGL canvas | `rust/extensions/thecrew/ui/src/canvas/SceneCanvas.tsx` (r3f `<Canvas>` + `<EffectComposer>`) |
| Prior twin bug (same error signature) | `docs/debugging/frontend/ce-page-crashes-openstream-detached-this-drops-shell.md` |

### Build note (IMPORTANT)
The thecrew extension UI does **NOT** hot-reload — it's a federated remote bundle served by the gateway.
If the fix ends up needing an extension-side change (`mount.tsx`, `SceneCanvas.tsx`), you must rebuild
thecrew's UI bundle and re-serve it (see `rust/extensions/thecrew/build.sh`), not just edit + save. A
shell-side change (`ExtWidget.tsx` etc.) DOES hot-reload in the :5173 dev server.

---

## 8. Test/verification state

- `ui/src/features/dashboard/builder/ExtWidget.test.tsx` — "defers the tile unmount out of the
  synchronous cleanup" is GREEN, but it only proves the *mechanism* (unmount is deferred). It does NOT
  prove the user's bug is fixed (jsdom has no WebGL and no real commit race). **Treat it as unverified.**
- Unit suite has 17 pre-existing failures in unrelated files (theme/agent/channel/app); not related.
- The gateway/e2e suites need a spawned node + built shell (`make ui-preview`) not available in my
  sandbox — the real proof must come from a browser.

---

## 9. Honest assessment

I fixed a REAL latent bug (synchronous nested `root.unmount()` during shell commit — the captured error
is genuine and the defer is a correct improvement), but I have **not proven it's the bug the user is
hitting**, and the user says it still breaks. The most likely miss is §5.5 — the tile being torn
down/remounted by a `scope`/`config`/refresh dep change and colliding with something — which I never
tested. Start at §5.1 (get the user's real console error); it will immediately tell you whether it's the
same `removeChild` commit error (my theory, defer harder) or something else entirely (wrong theory).
