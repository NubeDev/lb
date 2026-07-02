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

## slice-9.1 — the leaks the Preflight-only guard missed (2026-07-03)

The user pushed back: "99% sure there could STILL be CSS leakage from the CE extension." Correct. slice-9
closed the RESET and UTILITIES, but "no Preflight ≠ no leakage." A full global-write sweep of both
injected stylesheets (`scratchpad/sweep.mjs` — enumerate every `:root`/`html`/`body`/bare-selector/
at-rule write, not just Preflight signatures) found **three more** global writes, all of which the
`preflight-audit` guard passed clean:

1. **`:root,.ce-wiresheet{…}` token writes (HIGH)** in `wiresheet-theme.css` — 8 tokens (`--card` etc.)
   sharing the host's names, injected after the shell → editor's dark values overrode the shell's tokens
   document-wide (shadcn cards/popovers flipped dark in host light mode). Verified: host `--card:
   var(--panel)` vs editor `--card: 232 15% 9%`.
2. **`@theme → :root,:host{…}` (MED)** — `--color-*` + generic v4 vars (`--radius-md`, `--font-sans`,
   `--ease-out`, …) written to `:root`; `--radius-md`/fonts differ from the host's → shell-wide override.
3. **bare `.react-flow*` rules (HIGH)** from `@xyflow/react/dist/style.css` (~150 rules, some
   `@media`-nested) AND a **JS-injected** `<style>` (`CeEditor.tsx`'s `EDGE_SELECTED_CSS`) — both collided
   with the host's own React Flow canvases (system/data/flows views). The JS one lived in a template
   literal, so a CSS-file audit missed it; found by scanning the built JS chunk.

**Fixes (all upstream, S2):**

- `wiresheet-theme.css` — dropped `:root`; tokens `.ce-wiresheet`-scoped and host-inheriting
  (`--card: var(--card, <default>)` → tracks host light/dark, standalone fallback). Editor-only tokens
  (`--cool`/`--crit`/`--r1`/`--r2`) keep fixed defaults.
- `scope-css.ts` — a build-time Vite plugin (wired into `vite.lib.config.ts`, runs in `writeBundle`
  because `@tailwindcss/vite` + `cssCodeSplit:false` emits the CSS outside the rollup bundle) that
  rewrites the FINAL `ce-wiresheet.css`: `:root`/`:host` → `.ce-wiresheet` (deduped), every `.react-flow`
  selector (incl. `@media`-nested / element-prefixed / descendant) → `.ce-wiresheet`-prefixed. Leaves
  keyframe steps, `@property`/`@theme`/`@font-face`, and the `*,:before` `--tw-*` polyfill alone.
  Idempotent + brace-balanced. Unit-tested in `src/scope-css.test.ts` (6 tests).
- `CeEditor.tsx` — `EDGE_SELECTED_CSS` selector scoped at source (a CSS transform can't reach a JS string).

**Guard hardened:** `src/global-scope-audit.test.ts` asserts the built `ce-wiresheet.css` AND the
remoteEntry JS chunks carry zero `:root`/`:host` token writes and zero unscoped `.react-flow` CSS rules
(the JS check matches a `.react-flow…{prop:` rule, not a `querySelectorAll(".react-flow…")` string).
Verified to BITE (un-scoping `EDGE_SELECTED_CSS` fails it).

Final sweep — `:root` writes **0**, `.react-flow` **141 scoped / 0 unscoped**, JS-chunk editor `:root`
writes **0**. Green: control-engine/ui **35 | 2 skipped**, ce-wiresheet **153**.

Also audited (already safe, no change): `ui/styles.ts` (scoped under `.ce-ui-root`), `ClickDebugger.tsx`
(`@keyframes ce-ring-fade` — `ce-`-namespaced).

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
  `@tailwind base` + re-declared `:root`. Apply the same slice-9 + 9.1 fix (drop Preflight, scope
  utilities, `:root`→scope with host inheritance). If it vendors any Tailwind-v4 lib or `@xyflow`, audit
  for the `@theme :root,:host` + bare-vendor-rule leaks too. Optional here; lower urgency (smaller page)
  but the same latent leaks. slice-9 is the rule for every extension UI.
- **Live verification** of the slice-9.1 fixes specifically: open CE with the host in **light** mode and
  confirm the shell's cards/popovers/inputs stay light (the `:root --card` override was the visible
  symptom), and that opening CE doesn't re-theme the host's **system/data/flows** React Flow canvases
  (the `.react-flow` collision). Machine-checked by the audit; needs eyes to confirm.
