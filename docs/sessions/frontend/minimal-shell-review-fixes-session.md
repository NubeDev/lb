# Session — minimal-shell peer-review fixes (2026-07-11)

Independent review of 3c20433 (`scope 5: minimal-shell`) against
`docs/scope/frontend/minimal-shell-scope.md`, then fixes applied in place.

## Verdict at review: fix-then-ship

Rule 10 clean (ext id is opaque config; discovery via `ext.list`), rule 9 clean (no fakes;
real jsdom render tests), FILE-LAYOUT clean (largest file 166 lines). But three real client
bugs and several Goals-named pieces unbuilt.

## Fixed

1. **SSE subscribe 401** — `events.ts` sent no `Authorization` header to the header-authed
   `POST /events/{sid}/subscribe`. Both call sites fixed.
   → `docs/debugging/frontend/minimal-shell-sse-subscribe-401.md`
2. **Stale UI after 401** — `ipc.ts` cleared the stored session without notifying the store;
   added the `lb.session.cleared` event + `session.ts` listener. Regression test.
3. **`getSession` snapshot instability** — fresh `JSON.parse` per call → React
   `useSyncExternalStore` loop once logged in. Cached by raw string. Regression test.
4. Style: `import { useRef }` was at the bottom of `App.tsx`; moved to the top.

## Deferred (recorded in the scope doc's "Shipped + amendments" section)

Workspace pick, branding + boot-config fetch, actual publishing (still `private: true`),
and the mandatory e2e testing plan (capability-deny state, Playwright `hello`-fixture
mount, PWA, SSE reconnect, pre-auth branding paint).

## Test evidence

`cd packages/minimal-shell && pnpm test` → **4 passed (4)**.
