# Session: brand palette refresh (light/dark defaults → house teal + deep navy)

**Date:** 2026-07-05
**Area:** `ui/` theme system (frontend scope, theme-appearance scope)

## Ask

Improve the default light/dark colors. The house palette is the one the Professional
look carries: deep navy ink, clean paper, a deep-teal accent (teal brightened on dark).
Make that the out-of-box light/dark experience while keeping amber/blue as explicit
accent choices.

## What changed

- `ui/src/styles/globals.css`
  - `:root` (light): warm amber paper → clean cool paper (`--bg 210 20% 98.5%`), true-white
    panels, deep navy ink (`--fg 222 30% 16%`), default `--accent` is now the deep teal
    (`178 72% 27%`); `--accent-2` is the deep navy sibling; `--muted-bg`/`--panel-2`/`--overlay`
    retinted to the cool family.
  - `.dark`: neutral charcoal (hue 228) → deep navy near-black family (hue ~218, sat 14–22%),
    teal accent `174 62% 50%`, sky-blue `--accent-2`; dark shadows retinted to hue 220.
  - Added explicit `[data-theme-accent="amber"]` blocks (light + dark) so amber remains a
    first-class accent choice now that it is no longer the `:root` default.
- `ui/src/lib/theme/theme-options.ts`: `DEFAULT_THEME.preset` `"amber"` → `"teal"`.
- `ui/src/lib/theme/theme-looks.data.ts`: the `default` (Operator Console) look now defaults
  to the teal preset; blurb updated.
- `ui/src/lib/theme/theme-presets.data.ts`: the `slate` preset (Professional look) re-authored —
  light keeps clean paper but with deep navy ink and the deep-teal accent; dark is now a deep
  navy night mode (oklch hue ~252) with the teal accent, instead of a generic cool slate.
- `ui/src/lib/theme/contrast.test.ts`: the static amber mirror of globals.css replaced with a
  `BUILTIN_ACCENTS` map (amber/teal/blue share the base neutrals); added an AA suite over all
  three built-ins in both modes. This test is the lockstep guard for globals.css values.
- `ui/src/lib/theme/theme-storage.test.ts`: legacy-normalization expectation follows the new
  default preset.

## Contrast (verified)

All AA checks enforced by `contrast.test.ts` (fg-on-bg ≥4.5, fg-on-panel ≥4.5, accent-on-bg ≥3.0)
pass for every shipped look and all three built-in accents. Hand-checked ratios: light fg/bg 15.0,
muted/bg 5.9, teal/bg 5.2; dark fg/bg 15.7, muted/bg 7.8, teal/bg 9.7.

## Decisions

- Kept the accent-swap architecture (`data-theme-accent`) instead of adding a new preset: the
  teal simply became the `:root` default and amber moved to an explicit attribute block —
  zero component changes, existing prefs with `preset:"amber"` keep rendering amber.
- Dark ground is navy-tinted (not gray) so the surface itself carries the brand family; the
  alternative (neutral gray + teal accent only) was rejected as indistinguishable from the
  previous charcoal console.

## Follow-up in the same session: floating/inset sidebar alignment

The `floating`/`inset` sidebar variants inset the rail as an 8px-padded card, but the content
column (`SidebarInset`) stayed full-bleed — the page header's top edge and the status bar's
bottom edge overshot the rail card, so the two chrome bands never lined up. Fix: a cascade rule
in `globals.css` (`[data-slot="sidebar"][data-variant="floating"|"inset"] ~ [data-slot="sidebar-inset"]`)
gives the content column a matching 8px margin, radius, hairline border and shadow on desktop;
the rail-facing side keeps margin 0 because the rail container's own padding provides the gutter.
Mobile (Sheet rail) stays full-bleed. This mirrors upstream shadcn's `inset` treatment, done as a
sibling-selector cascade instead of peer-variant utility chains (deterministic, one place).

## Follow-up: agent dock header alignment

The dock's header used compact padding (`py-2`) while routed pages use `.page-header`
(`min-h-[3.75rem]`), so the two bottom hairlines sat at different heights across the split.
The dock header now uses the same band metrics (`min-h-[3.75rem]`, `bg-panel-2/80`, `py-2.5`).

## Tests

`cd ui && pnpm test` → 105 files / 653 tests green (includes the new built-in AA suite).
Nothing broke during the session; no debugging entry needed.
