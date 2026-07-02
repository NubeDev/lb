# Session — CE extension page blanked the shell (`openStream` detached-`this` crash)

- **Date:** 2026-07-02
- **Branch:** ce-node-wiring (per user: STAY on this branch)
- **Scope:** two reported symptoms on the live shell:
  1. "when I add an appliance nothing happens"
  2. "the main UI sidebar is hidden when I click on the control-engine extension sidebar"

The backend halves of #1 (grants stored under the bare user name vs `user:`-prefixed resolve; missing
`LB_GATEWAY_URL` in `make dev`) were **already fixed** before this session (commit `83711a6`). This
session covers the remaining **frontend** crash — which is what actually produced both visible symptoms:
the page threw during mount, so the wiresheet never rendered ("add appliance does nothing") **and** the
throw unwound the shell ("sidebar hidden").

## What was wrong

Console showed `Cannot read properties of undefined (reading 'bridge') at openStream` plus React
"synchronously unmount a root" / "removeChild: not a child" errors. Root cause in two layers — full
write-up in [../../debugging/frontend/ce-page-crashes-openstream-detached-this-drops-shell.md](../../debugging/frontend/ce-page-crashes-openstream-detached-this-drops-shell.md):

1. `CeEditor.tsx` invoked the injected transport's `openStream` through a **detached local**, so the
   real `BridgeTransport.openStream` (a prototype method reading `this.bridge`) got `this === undefined`
   and threw on mount. Hidden by the in-repo `MockTransport`, which **arrow-binds** its methods.
2. A federated extension renders **in-process against the shell's React**, so that render throw unwound
   the whole shell (nav + sidebar), and `ExtHost`'s synchronous cleanup then fought React's commit.

## Changes

- `packages/ce-wiresheet/src/CeEditor.tsx` — call `openStream` **as a method** (cast the transport, not
  the extracted function); add the missing stream-effect **cleanup** (`stream.close()` + null the
  module-level `streamRef`) so a remount re-arms instead of skipping (was → static canvas + leaked sub).
- `ui/src/features/ext-host/ExtErrorBoundary.tsx` (new) + wired in
  `ui/src/features/routing/createAppRouter.tsx` — a crash wall so an extension render throw stays inside
  the extension surface; the shell keeps rendering.
- `ui/src/features/ext-host/ExtHost.tsx` — guarded effect teardown (`unmount()` / `replaceChildren()`
  can't surface as React unmount/removeChild errors).
- `packages/ce-wiresheet/src/lib/transport.test.tsx` — regression `ThisReadingTransport` (prototype
  methods reading `this`); reset rest singletons in `afterEach`.

## Tests (green)

- `packages/ce-wiresheet` — full suite **146/146 pass**; typecheck clean. Verified the regression
  **fails before / passes after**: restoring the detaching call throws `Cannot read properties of
  undefined (reading 'marker')` — the exact shape of the production `reading 'bridge'`.
- `rust/extensions/control-engine/ui` — **28 pass / 2 skipped**.
- `ui` shell — the 17 failing unit tests are **pre-existing** (`Invalid Chai property:
  toBeInTheDocument`, a jest-dom matcher-setup issue), confirmed by re-running on a stashed clean tree;
  none are in the touched routing/ext-host files. My touched files typecheck clean (the shell's other
  `tsc` errors are the pre-existing `@types/react` JSX-typing mismatch).

## Rebuilt artifacts (so the fix ships)

The browser loads the **built** `remoteEntry.js`, not source, so rebuilt in order:
- `packages/ce-wiresheet` → `pnpm run build:lib` (the ext aliases this dist).
- `rust/extensions/control-engine/ui` → `vite build` → fresh `dist/remoteEntry.js`.

## Capability-deny / workspace-isolation note

This change is UI-only and adds **no new verb, route, or cap** — it fixes how the client *calls* an
existing injected transport. The cap-deny + workspace-isolation guarantees are unchanged: every canvas
action still rides `bridge.call` (host re-checked against install-scope ∩ grant), and `appliance.list`
is still workspace-walled by the host. The existing `bridge-transport.test.ts` exercises the deny path
(watch-arm failure → honest `closed`, static canvas, no throw).

## Follow-ups (named, not done)

- The first-boot appliance-seed still races gateway startup (cosmetic; documented pre-session).
- The `MockTransport` in `transport.test.tsx` still arrow-binds; the new `ThisReadingTransport` is the
  guard, but consider converting `MockTransport` to prototype methods so the primary double matches real
  shape too.
