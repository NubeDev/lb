# Session — slice-9 federated CSS isolation (CE page stops taking over the shell)

- **Date:** 2026-07-02
- **Branch:** ce-node-wiring-v2 (per user: STAY on this branch)
- **Scope:** `rust/extensions/control-engine/docs/slice-9-federated-css-isolation.md` — the CE federated
  page, once mounted, **took over the whole shell** (main nav + sidebar collapsed, page filled the
  viewport). Root cause is CSS, not React (the React render-throw takeover was a *separate* bug, fixed in
  the prior session — see [ce-page-shell-crash-openstream-session.md](ce-page-shell-crash-openstream-session.md)).

## What was wrong

A federated page renders **in-process against the host document**, and `remoteEntry.ts` injects its
compiled stylesheets into the host `<head>`. Two injected stylesheets carried Tailwind **Preflight** (the
global `*`/`html`/`body` reset), which re-reset the LIVE shell and collapsed the sidebar's flex layout:

1. `ui/src/styles/tokens.css` began with `@tailwind base;` (→ Preflight) and re-declared `:root` with a
   fixed amber accent (ignored the host's light/dark + teal/blue accent).
2. `packages/ce-wiresheet/dist/ce-wiresheet.css` (vendored, injected `?raw`) began with
   `@import 'tailwindcss'`, pulling Preflight **and** ~200 unscoped global utilities (`.flex`, `.border`,
   …) that also collided with the host's own Tailwind.

Full root-cause write-up:
[../../debugging/frontend/ce-page-css-preflight-leaks-into-shell.md](../../debugging/frontend/ce-page-css-preflight-leaks-into-shell.md).

## What I changed (the slice-9 three-part contract)

- **`ui/src/styles/tokens.css`** — dropped `@tailwind base`; scoped `components`/`utilities` under
  `.ce-page` via `@layer` (emits `.ce-page .flex { … }`); replaced the `:root` re-declaration with a
  `.ce-page { --bg: var(--bg, <default>); … }` fallback so it inherits the host `:root` when present
  (follows host light/dark + accent) and only falls back standalone.
- **`ui/src/Page.tsx`** — added `ce-page` to the page-root element's className.
- **`packages/ce-wiresheet/src/wiresheet.css`** (UPSTREAM, per S2 — editor fixes never patched in the
  extension) — switched `@import 'tailwindcss'` to `@import 'tailwindcss/theme.css' layer(theme)` +
  `@layer utilities { .ce-wiresheet { @tailwind utilities } }` (NO preflight, utilities scoped under the
  `.ce-wiresheet` root the editor already puts on its container **and** portal root). Re-built with
  `pnpm build:lib` and re-vendored (the ext UI aliases the built dist).

Mirrors `library-css-leaks-global-utilities.md`'s fix for the workspace `packages/*`, applied to the
standalone extension UIs that fix never touched.

## Audit (the required slice-9 step) — before/after on the BUILT artifacts

| file | `@layer base` | box-sizing reset | bare `.flex`/`.border`/… |
| --- | --- | --- | --- |
| `ce-wiresheet.css` (before) | 1 | present | present (global) |
| `ce-wiresheet.css` (after) | 0 | none | none — all `.ce-wiresheet`-scoped (113 refs) |
| `remoteEntry-*.js` (after) | 0 | none | none — all `.ce-page`-scoped |

The only `*,:before,:after` left in the built bundle is the v4 `@layer properties` polyfill (seeds
`--tw-*` custom-property fallbacks, applies NO reset — inert on the host). A bare
`-webkit-tap-highlight-color` also survives, but from **d3-drag/d3-zoom JS** setting it on the canvas
element at runtime — not a CSS reset; the guard keys on reset *structure*, not lone property names.

## Tests (rule 9 — real, no mocks)

Both new, in the ext UI vitest suite (read the real compiled output):

- `src/preflight-audit.test.ts` — built `dist/remoteEntry-*.js` + `packages/ce-wiresheet/dist/ce-wiresheet.css`
  carry zero Preflight signatures. Verified it BITES: re-adding `@tailwind base` to `tokens.css` fails it.
- `src/tokens-scope.test.ts` — compiles `tokens.css` through the real Tailwind-v3/PostCSS pipeline and
  asserts no `@layer base` and every utility rule is `.ce-page`-scoped. Also verified to bite.

```
$ ./node_modules/.bin/vitest run
 Test Files  6 passed (6)
      Tests  33 passed | 2 skipped (35)
```

Builds green: `packages/ce-wiresheet` `build:lib` + `control-engine/ui` `vite build`.

## Build note (environmental, not slice-9)

`build.sh` / `make dev CE=1` run `pnpm run build:lib`, whose pre-run deps-status check invokes
`pnpm install`, which currently hard-fails under the repo's `minimumReleaseAge` supply-chain policy
because an **unrelated** lockfile entry (`@opencode-ai/sdk@1.17.13`) was published within the cutoff
window. Worked around locally with `--config.minimumReleaseAge=0` (and `PNPM_CONFIG_MINIMUM_RELEASE_AGE=0`
for the hook). Pre-existing, out of slice-9 scope — flagged for whoever owns the lockfile refresh.

## Live verification (still owed by a human)

The running shell is the only thing that reveals this class of bug (jsdom never does). Remaining manual
step under `make dev EXTAGENT=1 DEVKIT_BUILDER=container CE=1`: open Control Engine and confirm the shell
nav + sidebar stay intact, the page occupies only its route, and toggling host light/dark + accent
re-themes the CE chrome. The built-artifact audit above is the machine-checkable proxy; the visual
confirmation needs eyes on a browser.

## Follow-up (named)

- `proof-panel` (`rust/extensions/proof-panel/ui/src/styles/tokens.css`) ships the identical
  `@tailwind base` + re-declared `:root`. Apply the same slice-9 fix. Optional here; lower urgency
  (smaller page) but the same latent leak. slice-9 is written as the rule for every extension UI.
