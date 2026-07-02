# Federated remote throws `process is not defined` on load (three.js in a Vite lib build)

**Symptom (browser):** opening the thecrew "Graphics" page in the built shell shows
`Could not load thecrew: process is not defined`. The sidebar nav slot appears, the gateway
serves `remoteEntry.js` with HTTP 200, but the remote's module eval throws before `mount` runs.

**Affected:** the thecrew remote specifically — it BUNDLES three.js / `@react-three/fiber` (the
federation payoff: only this remote carries the ~1 MB engine). Remotes that externalise everything
heavy (proof-panel) never hit it.

**Environment:** the built shell (`make ui-preview`, :4173) against a real node; found live —
the thecrew unit + gateway Vitest suites are all green, because jsdom/Node HAS a `process` global,
so the missing browser-time replacement is invisible to them.

## Root cause

three.js and `@react-three/fiber` read `process.env.NODE_ENV` at module-eval time (the standard
dev/prod branch). In a Vite **app** build Vite injects `process.env.NODE_ENV` automatically; in a
Vite **library** build (`build.lib`, which is how a federated remote is emitted) it does **not** —
the bare `process.env.NODE_ENV` survives into `remoteEntry.js`. A browser has no `process`, so the
first access throws `ReferenceError: process is not defined` and the whole remote fails to evaluate.

Confirmed: `grep -c 'process.env.NODE' dist/remoteEntry.js` → 9 before the fix, 0 after.

## Fix

Define the replacement explicitly in the remote's own `vite.config.ts` (the shell app build defines
it for its OWN graph; a federated remote must define its own):

```ts
export default defineConfig({
  // ...
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  build: { lib: { /* ... */ } },
});
```

`rust/extensions/thecrew/ui/vite.config.ts`.

## Prevention / follow-up

- Any extension remote that bundles a lib reading `process.env.*` (three.js, and many others) needs
  this `define` in its lib build. A shared federation-remote vite preset (a follow-up) should carry
  it by default so each new extension doesn't rediscover this live.
- The unit/gateway suites can't catch it (Node has `process`). The honest guard is the live-shell
  Playwright e2e (`ui/e2e/thecrew.spec.ts`) — it loads the real `remoteEntry.js` in a real browser.
