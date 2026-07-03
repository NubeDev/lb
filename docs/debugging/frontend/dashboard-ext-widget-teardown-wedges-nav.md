# Opening a dashboard with a graphics-canvas (thecrew) widget wedges navigation

- **Area:** frontend
- **Date:** 2026-07-03
- **Status:** fixed in tree — awaiting user confirmation in their dev browser (two prior "fixes" both
  reproduced for the user, so this is not called resolved until they verify).
- **Symptom (as reported):** "when I click on a dashboard page with this widget the navigation
  breaks." A dashboard whose cell mounts thecrew's `ext:thecrew/scene` (the WebGL/three.js graphics
  canvas) renders fine, but after that the shell nav stops responding — clicking another dashboard /
  sidebar item does nothing.

## First analysis (SUPERSEDED — see "The microtask defer was WRONG" below)

> This section captures the useful error signature but its conclusion (sync unmount during commit → fix
> by deferring) was the wrong fix and reproduced for the user twice. The real cause is an orphaned root
> from a StrictMode-unsafe async effect — see the corrected sections further down.

The first hypothesis — the tile *throwing* while disposing its WebGL context — was **wrong**. A
Playwright drive of the real dev shell (`:5173`, `#/t/acme/dashboards?…&d=scene-dash` → click "Rules")
captured the actual failure:

```
Warning: Attempted to synchronously unmount a root while React was already rendering. React cannot
finish unmounting the root until the current render has completed …
  at ExtWidget … at WidgetView … at Grid … at DashboardView
PAGEERROR: Failed to execute 'removeChild' on 'Node': The node to be removed is not a child of this node.
  at removeChildFromContainer … at commitDeletionEffectsOnFiber …
```

The dashboard-cell federation path (`ExtWidget`) mounts the tile **in-process**; the tile owns its OWN
React root (`createRoot(el)` inside its `mount`). `ExtWidget`'s `useEffect` cleanup called that root's
`unmount()` **synchronously**. But that cleanup runs *during the shell's own render/commit* — a route
change re-renders the shell, which unmounts this cell. A nested **synchronous** `root.unmount()` then
tears down the tile's DOM subtree while the shell's reconciler is mid-commit over the SAME nodes → React
double-removes a node → `removeChild: not a child` is thrown **inside React's commit phase**
(`commitDeletionEffectsOnFiber`), which a `try/catch` in our cleanup CANNOT catch. The shell's commit
aborts, so the route swap never completes and navigation wedges.

This is why the earlier `try/catch` + `ExtErrorBoundary` did nothing: the error is not a render throw and
not thrown in our callback — it's an uncaught pageerror inside React's own commit. It's the same
secondary-error signature documented in
[ce-page-crashes-openstream-detached-this-drops-shell.md](ce-page-crashes-openstream-detached-this-drops-shell.md)
("synchronously unmount a root while rendering" / "removeChild: not a child"), but here it's the PRIMARY
cause, triggered purely by the synchronous nested unmount (no upstream extension bug needed).

## The microtask defer (attempt 2) was WRONG — the real cause is an orphaned root

The first shipped fix deferred `root.unmount()` to a microtask and stopped hand-clearing the container.
**It did not fix the bug** (the user reported "100% NOT fixed" twice). Console tracing added to
`ExtWidget`'s effect captured the true sequence in the user's dev browser:

```
[ExtWidget] effect RUN  ext=thecrew …            ← run A
[ExtWidget] cleanup SCHEDULED ext=thecrew {hasTeardown: false, elConnected: true}   ← A cleaned up before its async mount resolved
[ExtWidget] effect RUN  ext=thecrew …            ← run B (same deps)
[ExtWidget] cleanup RUN (microtask) …            ← A's deferred teardown is a NO-OP (unmount was still undefined)
[ExtWidget] cleanup DONE
[ExtWidget] MOUNT ext=thecrew                    ← a root is created into the shared container
```

Root cause: **`ExtWidget`'s effect is async and was not StrictMode-safe.** `React.StrictMode`
(`ui/src/main.tsx`) double-invokes every effect in dev (mount → cleanup → mount). Because `mount()` is
`await`ed, run A was still loading its remote when its cleanup fired — so:

1. Run A's cleanup captured `unmount === undefined` (`hasTeardown: false`) → its deferred teardown was a
   no-op, and A's React root, **created later when A's promise resolved**, was never torn down.
2. Every run called `el.replaceChildren()` on the SAME container and mounted into it — so run B wiped
   the DOM that run A's `createRoot` still owned (or vice-versa), orphaning a live root.
3. On the real navigation-unmount, the orphaned root's `unmount()` removed nodes React had already
   removed → `removeChild: not a child` **inside the shell's commit** → commit aborts → nav wedges.

The microtask defer never addressed this: it changed *when* the (no-op or orphaned) unmount ran, not the
fact that a root was orphaned. This is why it reproduced only in dev (StrictMode) and only intermittently
(timing of the async mount vs. the double-invoke) — and why the Playwright repro was flaky.

## Fix (host-side — protects EVERY extension widget, not just thecrew)

Make the async mount effect **StrictMode-safe** by giving each effect-run its OWN child node and keeping
its teardown in the run's own closure:

```ts
const slot = document.createElement("div");   // a private node THIS run owns
host.appendChild(slot);
let alive = true;
const holder: { unmount?: () => void } = {};
(async () => {
  const mount = await loadRemoteWidgetMount(...);
  if (!alive) return;
  const teardown = mount(slot, ctx, bridge, widget);
  if (!alive) teardown?.();            // cleanup already ran while awaiting → tear down now (no leak)
  else holder.unmount = teardown;
})();
return () => { alive = false; holder.unmount?.(); slot.remove(); };
```

Why this fixes it:
- **Per-run slot:** run A and run B mount into DIFFERENT nodes, so neither can wipe the other's live
  root's DOM. No `replaceChildren()` on a shared container.
- **No orphan:** a `mount()` that resolves after cleanup sees `alive === false` and tears itself down
  immediately; a normal run stashes its teardown in `holder`, so cleanup always unmounts the root THIS
  run created — the `hasTeardown: false` no-op can't happen.
- **Synchronous unmount is now safe** (no microtask needed): the tile's `root.unmount()` only removes
  children of OUR `slot`, a node no other run and no shell commit touches — no shared-DOM double-remove.
  We remove `slot` ourselves; the shell only ever removes the whole ref'd container on real unmount.

Applied identically to both in-process mount points:
- [ui/src/features/dashboard/builder/ExtWidget.tsx](../../../ui/src/features/dashboard/builder/ExtWidget.tsx) (dashboard cell — the reported bug)
- [ui/src/features/ext-host/ExtHost.tsx](../../../ui/src/features/ext-host/ExtHost.tsx) (extension page — the latent identical bug)

`WidgetView` still wraps `<ExtWidget>` in `ExtErrorBoundary` (keyed on `ext:<id>/<widget>:<cell.i>`) —
a belt-and-braces crash wall for a genuine *render-time* tile throw, but it is NOT what fixed this bug.

## Regression

`ExtWidget.test.tsx` pins the real mechanism (all jsdom, no WebGL needed):
- **"mounts the tile into a private child slot, not the shared container"** — the node handed to the
  tile's `mount` is a child of the ref'd container, never the container itself.
- **"tears the tile down exactly once on unmount"** — no double-unmount.
- **"still tears down a tile whose mount resolves after cleanup already ran"** — the mock unmounts the
  component the instant its `mount` is entered (simulating cleanup racing the `await`), and the test
  asserts the resolved root is still torn down exactly once (the `alive === false` branch). This is the
  orphan-leak the old code had and the direct cause of the wedge.

## Lesson

An **async** effect that mounts an independently-`createRoot`ed subtree must be StrictMode-safe: because
StrictMode runs mount → cleanup → mount and the mount is awaited, cleanup can fire before the root
exists, and successive runs sharing one container will orphan each other's live roots. The orphan's later
`unmount()` double-removes DOM *inside the parent's commit* (`removeChild: not a child`), aborting the
render — uncatchable by any `try/catch`. Fix it structurally: **one private container node per
effect-run**, keep the teardown in that run's closure, and tear down a late-resolving mount if cleanup
already ran. Deferring the unmount (microtask/macrotask) treats the symptom, not the orphan — it will
look fixed in a green repro and still break for the user. And: add console tracing to the real dev
browser BEFORE theorizing — the `hasTeardown: false` log is what exposed the orphan two guesses later.
