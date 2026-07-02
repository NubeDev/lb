# frontend — a `packages/*` UI library's stylesheet leaked GLOBAL Tailwind utilities and broke the host app

Status: **resolved (2026-07-02)**. Area: frontend / shared UI packages (`@nube/panel`, `@nube/nav-rail`).

## Symptom

After wiring the new `@nube/panel` into `ui` (and importing `@nube/panel/style.css` in `main.tsx`,
alongside the existing `@nube/nav-rail/style.css`):

- the app's **left sidebar disappeared entirely** (visible with NO editor open — so not the panel
  component), and
- the dashboard Edit panel's **selected nav item ("Query") rendered as a fixed near-black block**
  in the host's *light* theme.

## Root cause

Two independent bugs in the shared packages' stylesheets, both from treating a Tailwind **library**
stylesheet like an **app** stylesheet.

1. **Unscoped utility leak.** Both packages shipped a top-level
   `@import 'tailwindcss/utilities.css'`, which emits ~200 **global** utility rules
   (`.flex`, `.grid`, `.border`, `.w-full`, `.hidden`, …). Dropped into a host that *also* ships
   Tailwind — and at a **different minor** (`ui` builds v4.1; `@nube/panel` built v4.3) — these
   duplicate class names collide and override the app's own utilities (different `@property`
   defaults / internals per version). The app's sidebar layout depended on the app's version of
   those utilities; the library's copy, imported *after* `globals.css`, won the cascade and broke
   it. nav-rail had leaked this way since it landed, but only one copy; adding panel's second,
   newer-version copy tipped the app over.

2. **Fixed palette, host theme ignored.** nav-rail's `NavMenu` selected item uses `bg-nr-bg`, and
   `--nr-bg` was a hard-coded near-black. It never followed the host's light/dark theme, so the
   active tab was a black block in light mode. (`@nube/panel` had the same fixed-palette shape in
   its first cut — fixed in the same session before this symptom.)

Neither showed up in tests: the package unit tests render the component in isolation (no host app to
collide with), and the app's unit/gateway suites run under jsdom where computed layout/color aren't
asserted. Only running the real app surfaced it.

## Fix

**Scope the utilities under the package root class** using Tailwind v4's nesting form, and drop the
global utilities import:

```css
/* panel.css / nav-rail.css */
@layer theme, utilities;
@import 'tailwindcss/theme.css' layer(theme);
@import './panel-theme.css';                 /* (or nav-rail-theme.css) */
@layer utilities {
  .lb-panel { @tailwind utilities; }         /* (or .nav-rail) */
}
```

Now every generated utility is emitted as `.lb-panel .flex` / `.nav-rail .flex` — it only applies
*inside* the component and can't touch app elements.

**Alias the theme tokens onto the host's shadcn vars** (with dark fallbacks for a standalone mount),
so the component follows the app theme instead of shipping its own palette:

```css
.nav-rail {
  --nr-bg:     var(--muted-bg, 234 18% 6%);
  --nr-panel:  var(--card, 232 15% 9%);
  --nr-fg:     var(--foreground, 220 16% 93%);
  --nr-muted:  var(--muted-foreground, 220 8% 56%);
  --nr-accent: var(--ring, 200 86% 64%);
  --nr-border: var(--border, 230 10% 18%);
}
/* @nube/panel: same shape onto --lbp-* (--lbp-panel: var(--card, …), etc.) */
```

## Regression guard

- Built `dist/*.css` must emit **scoped** utilities and **no** unscoped structural ones:
  `grep -oE '\}\.(flex|grid|border|w-full|hidden)\{' dist/panel.css` → empty;
  `grep -c '.lb-panel .flex' dist/panel.css` → ≥1. Same for `.nav-rail`.
- Preflight-free stays enforced: `grep -c '@layer base' dist/*.css` → `0`.
- Theme-follow: `grep -c 'var(--card' dist/panel.css` / `grep -c 'var(--card' dist/nav-rail.css` → ≥1.
- Green after fix: nav-rail 12/12, panel 7/7, `ui` unit 322/322, both editor gateway suites, `vite build`.

## Follow-on: scoping makes the root see-through

Scoping utilities under `.lb-panel` emits **descendant** selectors (`.lb-panel .bg-lbp-panel`).
The panel surface carries BOTH `.lb-panel` and `bg-lbp-panel` on the *same* element, so the
descendant selector never matches the root → the surface had **no background → transparent**.
Fix: set the root's own base look **directly on the class** in `panel-theme.css`, not via a utility:
`.lb-panel { background-color: hsl(var(--lbp-panel)); color: hsl(var(--lbp-fg)); }`. Descendant
utilities (header, borders, …) still apply because they're genuine descendants.

## Lesson

A Tailwind **library** stylesheet dropped into a Tailwind host must ship: **(1) utilities scoped
under its own root class** (never a global `@import 'tailwindcss'`/`utilities.css`), **(2) theme
tokens that ALIAS the host's shadcn vars** (never a fixed palette), and **(3) no preflight/base**.
Violating any of the three lets the library silently override or ignore the host — and an AI can't
see it in jsdom tests; only the running app reveals it. This generalises the earlier preflight-only
lesson (`react-types-19-collision.md` neighbours) to utilities and theme. See also: pin the library's
Tailwind to the app's version to avoid the cross-minor `@property` skew.
