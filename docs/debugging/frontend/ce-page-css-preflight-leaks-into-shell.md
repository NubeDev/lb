# frontend — the control-engine federated page's stylesheet shipped Preflight and took over the whole shell

Status: **resolved (2026-07-02)**. Area: frontend / federated extension UIs (`control-engine`, and the
contract for every extension UI). Slice: `rust/extensions/control-engine/docs/slice-9-federated-css-isolation.md`.

## Symptom

Opening the Control Engine extension page mounted the federated wiresheet, but the **whole shell
collapsed**: the main nav rail and the left sidebar disappeared and the CE page spilled to fill the
entire viewport (instead of occupying only its route surface). Nothing in the React tree was wrong — the
page rendered; the *host's layout* broke underneath it.

## Root cause

CSS, not React. A federated page renders **in-process against the host document** (S7: `ExtHost` calls
`mount(el, …)` into the shell's DOM), and `remoteEntry.ts` injects the page's compiled stylesheet into
the host `<head>` on first mount. Two of those injected stylesheets carried Tailwind **Preflight** — the
GLOBAL `*`/`html`/`body` reset (`box-sizing:border-box`, `margin:0`, `-webkit-text-size-adjust`,
unstyled headings/lists):

1. **`ui/src/styles/tokens.css`** (the page's own entry stylesheet, Tailwind v3) began with
   `@tailwind base; @tailwind components; @tailwind utilities;`. `@tailwind base` expands to Preflight.
2. **`packages/ce-wiresheet/dist/ce-wiresheet.css`** (the vendored editor's bundled theme, Tailwind v4,
   injected `?raw`) began with `@import 'tailwindcss'`, which pulls Preflight **and** ~200 GLOBAL utility
   rules (`.flex`, `.border`, …) unscoped.

Injected **after** the shell's own `globals.css`, these same-specificity global rules won the cascade and
**re-reset the live shell**: `body`/`*` margins and box-sizing changed under the running layout, the
shell's flex sidebar collapsed, and the page spilled full-width. The ce-wiresheet utilities also collided
`.flex`/`.border`/… against the host's own (independently-built) Tailwind — exactly the
`library-css-leaks-global-utilities.md` failure, this time for the standalone extension UIs that fix
never touched.

Secondary drift: `tokens.css` also **re-declared** `:root { --bg … --accent … }` with a fixed amber and
no `[data-theme-accent]` support, so the page ignored the host's teal/blue accent and light/dark swaps.

**Why tests never caught it.** The page/editor unit tests render in isolation (no host shell to collide
with), and the app's jsdom suites don't assert computed layout/color. Only the **running shell** revealed
it — the same blind spot as `library-css-leaks-global-utilities.md`.

## Fix — the federated-page CSS contract (slice-9)

A federated/injected stylesheet is a **library** stylesheet, not an app stylesheet. Three rules:

1. **No Preflight, ever.** The host owns the global reset. Dropped `@tailwind base` from `tokens.css`;
   switched ce-wiresheet's `wiresheet.css` from `@import 'tailwindcss'` to explicit
   `@import 'tailwindcss/theme.css' layer(theme)` + scoped utilities (NO `preflight.css`).
2. **Scope every generated utility under the page root.** `tokens.css` now emits
   `@layer utilities { .ce-page { @tailwind utilities } }` (→ `.ce-page .flex { … }`, inert outside the
   extension subtree); `Page.tsx`'s root element carries `.ce-page`. ce-wiresheet scopes its utilities
   under `.ce-wiresheet` (the class the editor already puts on its container **and** its portal root).
3. **Inherit host tokens; fallback only for standalone dev.** `tokens.css` no longer re-declares `:root`.
   It keeps a `.ce-page { --bg: var(--bg, <default>); … }` fallback — the self-reference resolves to the
   host `:root` when present (page follows host light/dark + accent for free) and to the default only when
   served standalone.

ce-wiresheet was fixed **upstream** in `packages/ce-wiresheet/src/wiresheet.css` and re-built
(`pnpm build:lib`), per the S2 rule (editor fixes go upstream, never patched in the extension).

## Regression guard (rule 9 — real)

Both live in the ext UI vitest suite (`pnpm test`), reading the REAL compiled output:

- **`src/preflight-audit.test.ts`** — asserts the built `dist/remoteEntry-*.js` chunks AND
  `packages/ce-wiresheet/dist/ce-wiresheet.css` contain ZERO Preflight signatures
  (`*,::before,::after` box-sizing reset, `html{-webkit-text-size-adjust:}`, `@tailwind base`,
  `@layer base`, `layer(base)`). Fails the moment `@tailwind base` returns. (Note: a bare
  `-webkit-tap-highlight-color` is NOT a signature — d3-drag/d3-zoom set it on the canvas element in JS;
  the guard keys on reset *structure*, not lone property names.)
- **`src/tokens-scope.test.ts`** — compiles `tokens.css` through the real Tailwind-v3 + PostCSS pipeline
  and asserts (a) no `@layer base`/Preflight is emitted, and (b) every generated utility rule is
  `.ce-page`-scoped — no bare `.flex`/`.border` at the rule root.

Measured before/after on the built artifacts:

| file | `@layer base` | box-sizing reset | bare `.flex`/`.border`/… |
| --- | --- | --- | --- |
| `ce-wiresheet.css` (before) | 1 | present | present (global) |
| `ce-wiresheet.css` (after) | 0 | none | none (all `.ce-wiresheet`-scoped, 113 refs) |
| `remoteEntry-*.js` (after) | 0 | none | none (all `.ce-page`-scoped) |

Green after fix: control-engine/ui **33 passed | 2 skipped**, `vite build` + `build:lib` clean.

## Lesson

A federated/injected stylesheet renders into the HOST document — it is a **library** stylesheet. Never
ship Preflight (the host owns the global reset), never emit unscoped global utilities (scope them under
the page/editor root), and never re-declare the host's `:root` tokens (inherit them, keep only a
standalone-dev fallback). This is the rule for **every** extension UI, not just CE — `proof-panel` has
the identical `@tailwind base` leak (see follow-up).

## Related

- `rust/extensions/control-engine/docs/slice-9-federated-css-isolation.md` — the full contract + rejected
  alternatives (shadow DOM, importing host CSS, injecting Preflight into `el`).
- `docs/debugging/frontend/library-css-leaks-global-utilities.md` — the first instance of this class,
  fixed for the workspace `packages/*`; slice-9 closes it for the standalone extension UIs.
- `docs/debugging/frontend/ce-page-crashes-openstream-detached-this-drops-shell.md` — a *different*
  CE-shell-takeover bug (a React render throw), fixed in the prior session; this one is pure CSS.

## Follow-up (named, not done here)

- `proof-panel` (`rust/extensions/proof-panel/ui/src/styles/tokens.css`) ships the identical
  `@tailwind base` + re-declared `:root`. Apply the same slice-9 three-part fix. Lower urgency (its page
  is smaller, so the takeover is less visible) but the same latent leak.
