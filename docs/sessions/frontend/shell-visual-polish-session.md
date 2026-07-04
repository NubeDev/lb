# Session — Shell visual polish + glass-intensity axis

**Date:** 2026-07-04
**Area:** frontend (theme + shell chrome)
**Scope touched:** theme-appearance (surfaces/tokens), dashboard, settings

## The ask

The theme *plumbing* (looks/tokens/motion/resolver/migration/ctx.theme v4) works and is
tested — leave it intact. The complaint was purely visual: the dark shell looked flat,
muddy, and unfinished, not like a premium operator tool (Linear/Stripe/Raycast bar). Make
it genuinely crisp and legible, dark-first, without leaning on glass/blur (the PRODUCT.md
anti-references reject glass theatrics for the default look). Verify in the real running
app with screenshots, keep the suites green.

Mid-session the user (viewing the dashboard with the **glass** surface active) added:
glass is "too much and the colours dont match well" — asked for a glass-intensity setting.

## What shipped

### Dark token ramp (globals.css `.dark`)
- Retuned the neutral family from a **warm charcoal** (hue 28–40) to a **cool near-black**
  (hue ~228, low chroma). The warm ground muddied every surface toward brown and fought the
  amber accent; a cool ground lets amber read as a deliberate signal color.
- Crisper plane stack: page `6%` → cell panel `9%` → raised rail/chrome `13%`, hairline
  border `20%`.
- **Shadows are now true black in dark mode**, not `hsl(var(--fg)/…)`. fg is near-white in
  dark, so every "shadow" rendered as a faint glow and flattened elevation. Added
  `.dark` + `:root.dark[data-surface="elevated"|"glass"]` blocks with black ambient shadows.
- Mirrored the new dark palette into `contrast.test.ts`'s `AMBER` block (kept AA green: 6/6).

### Glass-intensity axis (new — the user request)
A new **optional appearance axis** `glass: "subtle" | "medium" | "heavy"`, built to the exact
same pattern as the existing `surface`/`motion` axes (no resolver rewrite, no Rust change —
it rides the one persisted theme blob, serde-default flows through, verified by theme-prefs
gateway 6/6):
- `appearance-axes.ts` — `GLASS_LEVELS`, `DEFAULT_GLASS = "subtle"`, `isGlass`, `GLASS_OPTIONS`.
- `theme-options.ts` — optional `glass?` field + per-axis normalize (dropped if invalid).
- `theme-looks.data.ts` — `LookDefaults.glass`; the Liquid Glass look lands at `glass: "medium"`.
- `look-resolve.ts` — `ResolvedAppearance.glass`, folded through the precedence chain + reset
  in `applyLook`.
- `theme-dom.ts` — writes `data-glass` on `<html>`.
- `theme-context.ts` / `ThemeProvider.tsx` — `setGlass` setter.
- `GlassPicker.tsx` (new) — segmented control, **renders only when the resolved surface is
  `glass`** (nothing to tune otherwise). Wired into `ThemeTab` under the Surface picker.
- `globals.css` — the glass `@supports` block now tunes `--surface-alpha`/`--blur`/gradient
  per `data-glass` level. Default (subtle) is nearly opaque + light blur — crisp, per the
  anti-glass-theatrics rule.

### Glass color-clash fix (globals.css)
The glass panel gradient was `linear-gradient(135deg, accent/0.10, accent-2/0.06)` — i.e.
**amber → teal**, two different hues, which is exactly what read as "colours dont match."
Replaced with a **single-hue** accent wash fading to transparent, layered under a neutral
top light sheen (`fg/0.04`). Glass now reads as glass, not a muddy two-tone tint.

### Shell chrome
- **Nav rail** (`sidebar.tsx`): active pill switched from the secondary accent (`accent-2`,
  teal) to the primary accent — one signal color across the shell instead of a clashing
  second hue. Hover uses a neutral `fg/6%` wash. Group labels quieter (11px, `muted/80`).
- **Dashboard roster** (`DashboardRoster.tsx`): borderless rows, accent-tint selection,
  neutral hover; the `workspace` visibility tag is now hidden (it's the default — only
  non-default visibilities show).
- **Empty state** (`empty-state.tsx`): dropped the boxed dashed card (reads as a widget that
  failed to load on a big canvas) for a quiet centered icon+text stack (Linear-style).
- **Grid canvas** (`Grid.tsx`): faint dot-grid background marks the authoring surface;
  panel hover brightens toward `fg/25%` (neutral) instead of the teal `accent-2`.
- **Dashboard header** (`DashboardView.tsx`): the always-visible **Delete** demoted from a
  solid destructive button to a quiet ghost (destructive tone belongs to the confirm step).
- **Settings tabs** (`SettingsView.tsx`): the tab strip was a full-width boxed bar over a
  centered form — two competing layouts. Now `w-fit` + centered, sized to its pills.

## Testing (real app, screenshots)

Drove the running dev server (`:5173`, real gateway `:8080`) with Playwright as `user:ada` /
`acme`, screenshotting dashboards, settings, system, channels, datasources before/after, and
the three glass intensities. Verified live:
- The glass color clash is gone (single-hue sheen, no amber→teal).
- The **Glass intensity** control appears under Surface only when Liquid glass is selected,
  and writes `data-glass` on `<html>` (confirmed `subtle`/`medium`/`heavy`).
- The redesigned empty state + centered settings tabs render as intended.

Screenshots (scratch): dashboards, settings, glass-control-on, dash-glass-heavy.

## Suites

- `pnpm test` — **533 passed** (was 532; +1 for the new glass-axis assertions in
  `theme-dom.test.ts` / `look-resolve.test.ts`).
- `pnpm test:gateway theme-prefs` — **6/6** (glass field round-trips through real prefs sync).
- `pnpm lint` — 8 errors, all the pre-existing raw-`<button>` baseline (none new).
- `tsc --noEmit` — the 3 pre-existing FlowsCanvas/transformDebug errors (none new).

## Notes / rejected alternatives

- **Why an axis, not a CSS-only tweak for the intensity control?** The user asked for a
  *setting*. Adding a persisted axis follows the shipped surface/motion pattern exactly and
  needs zero Rust (one theme blob, serde-default). A one-off CSS constant wouldn't be a
  member-tunable setting.
- **Left the whole theme resolver/migration/ctx.theme v4/`lib/motion` seam untouched** — the
  glass axis only *extends* the per-axis fold, it doesn't change existing behavior (all prior
  theme tests still pass unchanged).
- No debugging entry — nothing broke; this was additive polish.
