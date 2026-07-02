# frontend — the control-engine federated page's stylesheet shipped Preflight and took over the whole shell

Status: **resolved (2026-07-03)** — Preflight fixed 2026-07-02 (slice-9), the remaining global leaks
fixed 2026-07-03 (slice-9.1, see "§ The leaks the first guard missed" below). Area: frontend / federated
extension UIs (`control-engine`, and the contract for every extension UI). Slice:
`rust/extensions/control-engine/docs/slice-9-federated-css-isolation.md`.

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

## The leaks the first guard missed (slice-9.1, 2026-07-03)

"No Preflight" ≠ "no leakage." The slice-9 fix closed the global RESET and the unscoped Tailwind
UTILITIES, but a follow-up sweep (`grep` every global write in both injected stylesheets, not just
Preflight signatures) found **three more** global writes the editor still shipped — all of which the
Preflight-only guard passed clean:

1. **`:root,.ce-wiresheet{…}` token writes (HIGH).** `wiresheet-theme.css` wrote the editor's palette
   (`--card --border --foreground --input --muted --muted-foreground --secondary --background`) to
   `:root` — the SAME names the host's `globals.css` defines. Injected after the shell's CSS, the
   editor's **dark** values (`--card: 232 15% 9%`) overrode those 8 tokens **document-wide**, so opening
   CE re-themed every shadcn card/popover/input in the whole shell toward dark, even in host light mode
   (verified: host `--card: var(--panel)` vs editor's fixed dark). Same class as the earlier
   `library-css-leaks-global-utilities.md` fixed-palette bug.
2. **Tailwind v4 `@theme` → `:root,:host{…}` (MED).** The editor's `@theme` block emitted `--color-*`
   plus generic v4 vars (`--spacing --radius-md --font-sans --font-mono --ease-out
   --default-transition-*`) to `:root,:host`. `--radius-md` (editor `.375rem` vs host
   `calc(var(--radius) - 2px)`) and the fonts DIFFER from the host's, so they overrode shell-wide.
3. **Bare `.react-flow*` rules (HIGH), from TWO sources:**
   - `@xyflow/react/dist/style.css` (imported by `CeEditor`) ships ~150 unscoped `.react-flow*` rules
     (some nested in `@media`). The host renders React Flow in its **system/data/flows** views with its
     OWN `.react-flow` theming (11 rules in `globals.css`) — last-injected wins, so opening CE re-themed
     the host's canvases.
   - A **JS-injected** `<style>` string in `CeEditor.tsx` (`EDGE_SELECTED_CSS`) with an unscoped
     `.react-flow__edge.selected` rule. A `<style>` applies DOCUMENT-WIDE regardless of where the element
     sits, so it re-colored the host's selected edges too. (This one lived in a JS template literal, not
     the CSS asset — invisible to a CSS-file audit; found only by scanning the built JS chunk.)

**Fixes (all upstream in `packages/ce-wiresheet`, per S2):**

- `wiresheet-theme.css` — dropped the `:root` selector; tokens now `.ce-wiresheet`-scoped and written as
  `--card: var(--card, <default>)` so they **inherit** the host token when present (editor tracks host
  light/dark — "look native") and fall back only standalone. Editor-only tokens (`--cool --crit --r1
  --r2`) keep fixed defaults.
- A build-time Vite plugin (`scope-css.ts`, wired into `vite.lib.config.ts`) rewrites the FINAL emitted
  `ce-wiresheet.css`: `:root`/`:host` → `.ce-wiresheet` (covers the `@theme` block + any stray), and
  every `.react-flow*` selector (incl. `@media`-nested, element-prefixed, descendant) → prefixed with
  `.ce-wiresheet`. Leaves keyframe steps, `@property`/`@theme`/`@font-face`, and the `*,:before` `--tw-*`
  polyfill untouched. Idempotent + brace-balanced (unit-tested in `src/scope-css.test.ts`).
- `CeEditor.tsx` `EDGE_SELECTED_CSS` — selector scoped to `.ce-wiresheet .react-flow__edge.selected …`
  at source (a build-time CSS transform can't reach a JS string).

**Guard hardened** (the Preflight-only guard would have passed all three): new
`src/global-scope-audit.test.ts` asserts the built `ce-wiresheet.css` AND the remoteEntry JS chunks carry
(a) zero `:root`/`:host` token writes and (b) zero unscoped `.react-flow` CSS rules (the JS-chunk check
matches a `.react-flow…{prop:` CSS rule, not a `querySelectorAll(".react-flow…")` JS string). Verified to
BITE: un-scoping `EDGE_SELECTED_CSS` fails it. Final sweep: `:root` writes **0**, `.react-flow` scoped
**141 / unscoped 0**, editor `:root` writes in the JS chunk **0**.

Green after slice-9.1: control-engine/ui **35 passed | 2 skipped**, ce-wiresheet **153 passed**,
`build:lib` + `vite build` clean.

## Lesson

A federated/injected stylesheet renders into the HOST document — it is a **library** stylesheet. It must
ship **nothing global**: no Preflight (the host owns the reset), no unscoped utilities, no `:root`/`:host`
custom-property writes (inherit host tokens, keep only a standalone fallback), and no unscoped element/
vendor rules (`.react-flow*`) — scope everything under the page/editor root. And "global" includes CSS
built in **JavaScript**: a `<style>` injected from a JS template literal applies document-wide no matter
where the element sits, so its selectors must be scoped too — and a CSS-file audit will miss it (scan the
built JS chunk). Crucially: **"no Preflight" is not "no leakage"** — a guard that only greps Preflight
signatures passes a `:root` token write and a bare `.react-flow` rule clean. Sweep for *every* global
write (`:root`, `html`/`body`, unscoped element/class rules, `@keyframes` names, JS-injected `<style>`),
not just the reset. This is the rule for **every** extension UI, not just CE — `proof-panel` has the
identical `@tailwind base` + `:root` leak (see follow-up).

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
