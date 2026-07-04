# Frontend scope — theme appearance (looks, fonts, surfaces, motion)

Status: scope (the ask). Successor to the shipped `theme-customizer-scope.md`. Promotes to
`public/frontend/frontend.md` once shipped.

"10x the theme." The shipped Customizer changes *colors*; this scope makes the whole **look and
feel** of the shell a member preference: one-click **look packs** (Code Editor, Professional, Retro
Terminal, Modern Dashboard, Liquid Glass), **font** choice, **surface treatments** (translucency,
blur, elevation, gradients), an app-wide **motion system** (motion.dev, with an off switch), and a
**wider tone palette** than today's two-step bg/panel. It also fixes two shipped Customizer bugs —
the radius control that visibly does nothing and the brand-color swatches that are not clickable —
and carries **extensions** along through the generic theme-inheritance seam, never a special case.

## Goals

- **Look packs.** A curated set of named, one-click looks, each a pure **data bundle** of defaults
  across every appearance axis (palette preset, font pair, radius, surface treatment, motion
  profile, density). Ship at least: `default` (the amber operator console), `editor` (code-editor:
  mono-forward, dense, sharp corners, muted syntax-like palette), `professional` (calm neutrals,
  generous whitespace, subtle elevation), `retro` (terminal: phosphor green/amber on near-black,
  square corners, scanline-free — a look, not a gimmick), `modern` (airy dashboard: soft gradients,
  large radius, elevated cards), `glass` (liquid glass: translucent panels, backdrop blur, layered
  elevation, gradient accents). A look is data in one file; adding a look adds **zero** branches.
- **Fonts.** First-class `--font-sans` / `--font-mono` tokens with a curated, **self-hosted**
  family list (system default stays the default; no CDN — the shell must boot offline and inside
  Tauri). Fix the dangling `var(--font-mono, …)` reference (`JsonTree.tsx`) by actually defining
  the token, and route the ad-hoc inline monospace stacks through it.
- **Surfaces.** A `surface` axis — `flat` (today) | `elevated` | `glass` — expressed as new tokens
  (`--surface-alpha`, `--blur`, `--shadow-1/2/3`, `--gradient-accent`) plus a `data-surface`
  attribute on `<html>`, so panels/cards/sheets/nav pick it up from CSS alone and extensions
  inherit it by cascade. Glass degrades gracefully where `backdrop-filter` is weak (WebKitGTK).
- **Motion.** A `motion` axis — `off` | `subtle` | `full` — honored two ways: a `data-motion`
  attribute that gates all CSS transitions/`tw-animate-css`, and a `useMotionPref()` seam that
  gates the new **motion.dev** (`motion` npm) animations in shell chrome (nav rail, sheets, tab
  and page transitions, accordion, hover elevation). `prefers-reduced-motion` always wins.
- **More tones.** Widen the base palette beyond the seven tokens: a raised-panel step
  (`--panel-2`), an overlay tone (`--overlay`), a secondary accent (`--accent-2`), and semantic
  `--success` / `--warning` alongside the existing destructive — so presets and looks can express
  more than a two-tone surface. Existing stored themes must keep working (see migration).
- **Fix the shipped bugs** (slice 0, before any new surface):
  - **Radius does nothing.** Only `--radius-sm/md/lg` derive from `var(--radius)`
    (`globals.css:38-40`); the app has ~114 uses of bare `rounded`, 38 `rounded-full`, and
    `rounded-xl` — all bound to Tailwind's static defaults. The picker writes the token correctly
    (`theme-dom.ts` is tested); the CSS never reads it. Fix = derive the full used scale from
    `--radius` in `@theme` and sweep bare `rounded` onto token-derived stops (`rounded-full` stays
    literal). Verify **live**, not just in jsdom.
  - **Brand colors not clickable.** The swatch is a native `<input type="color">`
    (`components/ui/color-picker.tsx`): only the 24×32 px swatch is interactive (the row is not),
    and WebKitGTK — the Tauri Linux webview — ships without native color-input support, so the
    click is a **no-op on desktop**. Fix = replace with a hand-authored, token-bound in-DOM
    popover picker (hue/saturation/lightness controls + hex field), whole row clickable, no
    platform dependency, no new heavyweight dep.
- **Extensions re-theme with all of it.** Every new token/attribute rides the *same* channels the
  theme-inheritance scope defines (CSS-var cascade in-process; injected vars + `lb.theme`
  postMessage for the iframe tier; resolved values in `ctx.theme` for canvas widgets). This scope
  **implements** that seam rather than forking it — see Intent.

## Non-goals

- No per-component theme editor, no arbitrary CSS injection surface (Import already covers the
  power user), no theme marketplace, no extension-*contributed* looks (additive later scope — a
  look is opaque data, so the registry path is natural when we want it).
- No custom font upload; the list is curated and bundled. No variable-font axis editor.
- No animated wallpapers/particles; motion is *interaction* motion, not decoration.
- No dashboard-widget animation pass — charts keep their own animation (ECharts owns it); motion
  here is shell chrome only.
- No workspace-level *lock* of appearance (same posture as the customizer scope: workspace default
  + member override).

## Intent / approach

**Everything is still one `ui_theme` blob — zero backend change.** The prefs axis added for the
customizer stores the whole `ThemePreference` as one opaque JSON value, so widening the TS shape
(`look`, `font`, `surface`, `motion`, extra palette tokens) is a client-side schema change plus
`normalizeThemePreference` defaults. No new verb, table, capability, or Rust change. (Rejected:
sibling prefs axes per new knob — churns the closed `lb_prefs::Prefs` struct for no isolation win;
the blob is already atomic and versioned by normalization.)

**A look is defaults, not a lock.** `ThemePreference` gains `look: string`; resolution is
per-axis: **explicit member choice → look default → built-in default** — the same fold shape the
prefs chain already taught us. Picking a look *resets* the axes the look defines (with the current
values recoverable via Reset semantics), because "I picked Retro but my old radius stuck" reads as
broken; a look must land looking like its thumbnail. Hand-tweaks after picking a look are normal
per-axis overrides on top of it. Looks live in `lib/theme/theme-looks.data.ts` as data
(FILE-LAYOUT: data, not branches); the resolver stays pure and unit-testable like
`theme-resolve.ts` today. (Rejected: look-as-mega-preset that deep-copies everything into
`custom` — loses the "what look am I on" identity and makes later look updates un-adoptable.)

**Fonts are tokens.** `--font-sans`/`--font-mono` are defined in `:root`, mapped through
`@theme inline` so Tailwind's `font-sans`/`font-mono` utilities resolve them at point of use, and
written by `theme-dom.ts` like any other token. Families ship self-hosted via `@fontsource-*`
woff2 packages (flag in `key-stack.md`), lazily loaded on first selection and preloaded when the
stored theme names one — system stack remains the zero-cost default. Curated starter list: Inter,
Geist, IBM Plex Sans (sans); JetBrains Mono, IBM Plex Mono (mono); one serif for `professional`
(Source Serif 4). Each family is a data row (label + `@fontsource` id + stack), not a branch.

**Surfaces are a cascade, not a component sweep.** `data-surface` on `<html>` + token blocks in
`globals.css` restyle `--panel`-consuming chrome (cards, sheets, popovers, nav) through the vars
they already read: glass = `--panel` gains alpha (`hsl(var(--panel) / var(--surface-alpha))`) +
`backdrop-filter: blur(var(--blur))` + `--shadow-*` elevation ramp; `elevated` = opaque panels +
the shadow ramp; `flat` = today. Guarded by `@supports (backdrop-filter: blur(1px))` with a solid
high-contrast fallback — WebKitGTK's compositor makes large blur regions expensive, so the
desktop shell may resolve `glass` to `elevated` via the same fallback path (a config/capability
degrade, never a code branch on platform in core components).

**Motion is one preference, two enforcement points.** (1) `data-motion` on `<html>`: `off` sets a
global `transition: none`/`animation: none` fence (and is forced by `prefers-reduced-motion`
unless the member explicitly chose `full`); CSS-only micro-transitions stay CSS. (2) A
`useMotionPref()` hook + a thin `lib/motion/` wrapper over **motion.dev** for the springy stuff
(sheet slide, accordion height, nav-rail collapse, page fade/slide, staggered list mounts) —
`subtle` = short, small-distance variants; `full` = the designed set. The wrapper is the only
import site of `motion` (one seam to tree-shake, one place extensions are told to mirror).
(Rejected: framer-motion — same library's legacy React-only packaging; `motion` is its successor
with a smaller hybrid engine. Rejected: CSS-only everything — no interruptible springs or layout
animations, which is most of what makes the shell *feel* better.)

**Tone widening with a compatibility fold.** New tokens join `BASE_TOKENS`, but `isBasePalette`'s
every-key-present rule would silently drop every stored custom/imported theme. So validation
splits into **required** (the shipped seven) and **derivable** (new tones, defaulted from the
required ones — `--panel-2` nudged from `--panel`, `--overlay` from `--bg`, `--accent-2` from
`--accent` — the same relative-derivation trick the preset adapter already plays). A stored theme
missing new tokens normalizes by derivation, never fails closed to DEFAULT_THEME.

**Extensions: implement the inheritance seam, widened.** The `theme-inheritance-scope.md`
contract (`lb:themechange` emitter in `lib/theme`, `ext-host` as sole subscriber, `ctx.theme` for
canvas widgets, injected vars for a future iframe tier) is currently **proposed, not built** —
widgets like thecrew self-read host vars with a `MutationObserver` today. This scope ships the
emitter + `ctx.theme` (additive `WIDGET_CTX_V = 4`, moved in all three mirrors together:
`federationWidget.ts`, the devkit template, extension contract copies) with the **widened**
resolved shape: base tokens + new tones + `radius` + `fontSans`/`fontMono` + `surface` + `motion`
+ the chart ramp. One contract bump instead of two back-to-back. The core never names an
extension; every consumer gets the same signal (rule 10).

## How it fits the core

- **Tenancy / isolation:** unchanged from the customizer — the whole preference is the member's
  own `ui_theme` prefs axis; workspace isolation is inherited from the `prefs` crate and re-proven
  by the mandatory test.
- **Capabilities:** unchanged — `mcp:prefs.set:call` to persist own, admin-gated
  `mcp:prefs.set_default:call` for the workspace default, opaque deny degrades to local-only. No
  new grant. (Dev-login still lacks `set_default` — seed an admin via `signInWithCaps` in tests.)
- **Symmetric nodes / placement:** UI-only; browser and Tauri run the same code. The glass→elevated
  degrade is a runtime `@supports`/quality fallback, not an `if desktop` branch.
- **One datastore:** the durable preference stays in the member's SurrealDB prefs record;
  localStorage remains only the first-paint cache (now also caching look/font/surface/motion so
  there is no flash of unstyled font or flat-to-glass pop).
- **No mocks:** persistence round-trips against the **real** spawned gateway (`pnpm test:gateway`);
  pure resolvers/normalizers in `pnpm test`. No `*.fake.ts`.
- **State vs motion (bus):** N/A — no Zenoh subject; theme is state, applied locally.
- **Stateless extensions:** extensions consume the signal, own nothing durable. No extension-id
  branch anywhere in the fan-out (the emitter has exactly one subscriber: `ext-host`).
- **MCP surface / API shape:** no new verbs. Reads via `prefs.resolve`, the single write via
  `prefs.set` (§6.1: get + update only; live feed and batch N/A as before).
- **Durability:** N/A — no cross-node must-deliver effect.
- **One responsibility per file:** `lib/theme/` grows `theme-looks.data.ts`, `theme-fonts.data.ts`,
  `look-resolve.ts`, `font-dom.ts`/`surface-dom.ts` (or fold into `theme-dom.ts` while under the
  line cap), `theme-events.ts` (the emitter); `lib/motion/` owns the motion seam;
  `features/theme/` gains `LookPicker.tsx`, `FontPicker.tsx`, `SurfacePicker.tsx`,
  `MotionPicker.tsx`; the popover color picker replaces `color-picker.tsx` in place. Looks and
  fonts are data files, never branches.
- **SDK/WIT impact:** the WASM ABI is untouched. The **UI federation widget contract** takes an
  additive `WIDGET_CTX_V` bump (3 → 4, `ctx.theme`) — flagged loudly: it must move in all three
  mirrors in one slice, and `data:true` widgets must keep working against a v3 host copy
  (additive-only, version-gated).
- **New deps (key-stack.md rows):** `motion` (motion.dev, MIT, ~18 kB hybrid engine) and
  `@fontsource-*` packages (self-hosted woff2). Both UI-only.

## Example flow

1. A member opens Settings → Theme. Above the preset picker sits **Look**: six thumbnail cards.
   They pick **Liquid Glass**.
2. The look resolver folds `glass`'s bundle over the built-in defaults: preset stays their choice,
   `surface: glass`, `radius: 0.75rem`, `font: inter`, `motion: full`. `theme-dom.ts` writes the
   tokens, `data-surface="glass"`, `data-motion="full"`, and the font tokens; panels go
   translucent with backdrop blur and layered shadows in one paint.
3. `lib/theme` emits `lb:themechange`; `ext-host` resolves the computed tokens once and fans out —
   an ECharts widget re-colors via `update(ctx)` with the new `ctx.theme`, no re-mount.
4. They flip the mode to light; the glass alpha/shadow tokens re-resolve from the light block, the
   custom-theme rule (mode flip re-applies the right variant) still holds.
5. They open **Brand colors**, click anywhere on the "Accent" row; the popover picker opens (also
   on the Tauri desktop, where the old native input silently did nothing), they drag hue, the app
   re-themes live per keystroke.
6. They nudge **Radius** to `1` — and now every card, input, chip, and button visibly rounds,
   because the whole `rounded-*` scale derives from `--radius`.
7. They set **Motion** to `subtle`; sheets and the nav rail keep short eased transitions, the
   staggered mounts stop. Their OS `prefers-reduced-motion` would have forced `off` by default.
8. The debounced `prefs.set` persists the whole preference; their desktop shell resolves the same
   theme on next boot, fonts preloaded from the stored choice — the look roamed.

## Testing plan

Per `scope/testing/testing-scope.md` — the customizer's suite is the floor; new coverage:

- **Slice-0 regressions (the two bugs):**
  - *Radius coverage:* a build-level guard that the compiled CSS derives the full used `rounded`
    scale from `var(--radius)` (assert on the built stylesheet or the `@theme` source), plus a
    sweep test that no `.tsx` under `ui/src` uses bare `rounded`/`rounded-xl` outside the
    allowlist (`rounded-full`, `rounded-none`). Live-verify the picker in the running app
    (`/verify` discipline), since jsdom cannot see Tailwind output.
  - *Color picker:* interaction tests — click the row → popover opens; choose a color → `onChange`
    fires with a valid triplet; keyboard operable; works with zero native `<input type="color">`
    dependency.
- **Look resolver (unit):** each shipped look folds to its exact expected axis set; explicit member
  overrides win per-axis; picking a look resets the axes it defines; unknown look id falls back to
  `default` (fail-open to data, closed to garbage).
- **Normalization/migration (unit):** a stored v-customizer preference (no look/font/surface/
  motion, seven-token custom theme) normalizes to working defaults **without** dropping the custom
  palette — the derivable-token fold. Malformed new fields fall back per-axis, never whole-blob.
- **DOM application (unit):** `data-surface`/`data-motion`/font tokens written and cleared
  correctly; `prefers-reduced-motion` forces motion off unless explicitly `full`.
- **Emitter/fan-out (unit + component):** a theme change emits exactly one `lb:themechange`;
  `ctx.theme` resolves the widened shape; a `data:true` widget receives an `update` with new theme
  (the WidgetHost harness — same rig as the frames-in contract tests).
- **Prefs round-trip (real gateway, mandatory):** the widened blob set + resolved back on a real
  node; second boot restores it.
- **Capability deny (mandatory):** persist without `mcp:prefs.set:call` → opaque deny, local-only;
  non-admin `prefs.set_default` → denied honestly.
- **Workspace isolation (mandatory):** two seeded members/workspaces, no cross-read/write of
  `ui_theme`.
- **A11y/contrast:** AA check on each shipped look's fg-on-surface pairs in both modes — glass
  especially (text over translucency); the contrast policy for looks is *ship-vetted* even while
  imports stay warn-only.
- **Build/lint:** `pnpm build`, `pnpm lint`, bundle-size check that `motion` + fonts stay behind
  lazy seams (fonts must not load unless selected).

## Risks & hard problems

- **Glass legibility and perf.** Translucent panels over arbitrary content is where contrast dies
  and WebKitGTK compositors crawl. The alpha/blur values are per-mode *tokens* (tunable data), the
  fallback ladder (`glass → elevated → flat`) is mandatory, and AA on shipped looks is a test, not
  a review note.
- **The radius sweep is wide but shallow.** ~120 call sites change class strings; the risk is
  visual regressions in dense chrome (tables, chips). The sweep guard test keeps it from
  regressing; do it as its own commit for reviewability.
- **Token widening vs. stored themes.** The strict `isBasePalette` fold is the one place this can
  silently nuke a member's hand-tuned palette. The derivation fold + its migration test are
  load-bearing; treat a fail-closed drop of a v1 custom theme as a blocker.
- **Look/override precedence confusion.** "I changed accent, then picked a look, where did my
  accent go?" is the UX trap. Picking a look resets its axes (decisive, thumbnail-true) — say so
  in the UI (one-line hint), and keep Reset semantics obvious.
- **Contract bump discipline.** `ctx.theme` must land in all three mirrors in one slice or
  devkit-scaffolded extensions drift; the version gate must keep v3-shaped hosts/copies working.
- **Motion sprawl.** Without the single `lib/motion` seam, `motion` imports leak everywhere and
  the off switch stops being trustworthy. Enforce one import site (lint rule or review line).
- **Font weight (kB).** Each family is ~100–300 kB of woff2; lazy-load on selection, preload only
  the stored choice, subset to latin by default.

## Open questions

- **Final font list.** Recommendation above (Inter/Geist/IBM Plex/JetBrains Mono/Source Serif 4) —
  confirm licensing posture (@fontsource = OFL) and whether the retro look wants a pixel/CRT face
  or stays JetBrains Mono (leaning: no novelty face; retro is palette + shape, fonts stay legible).
- **Do looks pin the color preset or only default it?** Leaning: default it (member's preset
  survives a look switch when they've explicitly chosen one; `retro` is the exception that pins,
  since its identity *is* the palette — expressed in data via a `pins: ["preset"]` field, still no
  branch).
- **Semantic `--success`/`--warning` now or later?** They ride the tone widening cheaply, but
  consumers (badges, telemetry states) are a separate sweep. Leaning: define tokens + derivations
  now, sweep consumers opportunistically.
- **Density axis** (compact/comfortable spacing) — `editor` wants it; is it this slice or a
  follow-up? Leaning: follow-up; it's a spacing-token sweep with wide blast radius, and looks work
  without it.
- **Does the iframe extension tier ship here?** The injected-vars + postMessage path is specified
  but the tier itself doesn't exist yet (`ExtHost.tsx` note). Leaning: implement the emitter such
  that the iframe fan-out is a subscriber away, but don't build the tier in this scope.
- **Motion default for new members:** `subtle` or `full`? Leaning `subtle` (respect first
  impressions and low-end hardware; `full` is a choice).

## Related

- `theme-customizer-scope.md` — the shipped predecessor: base-token bridge, prefs `ui_theme` axis,
  member/workspace-default fold. This scope widens its preference shape and fixes its two shipped
  bugs (radius coverage, color-picker platform support).
- `theme-switcher-scope.md` — the original accent switcher (history).
- `../extensions/ui/theme-inheritance-scope.md` — the live re-theme contract for extension
  pages/widgets that this scope **implements** (emitter + `ctx.theme` v4, widened shape);
  `../extensions/ui/css-isolation-scope.md` — the cascade fence that makes in-process inheritance
  safe (its rules are load-bearing here).
- `ui-standards-scope.md`, `ui-design-scope.md` — the token discipline and the dark-first amber
  identity the `default` look must preserve.
- `../prefs/user-prefs-scope.md`, `../../public/prefs/prefs.md` — the resolve chain and verbs
  persistence rides; README §6.6 for the gating grants.
- `../../FILE-LAYOUT.md` — data-not-branches decomposition for looks/fonts; one seam for motion.
- `../../key-stack.md` — add rows for `motion` (motion.dev) and `@fontsource-*` (self-hosted
  fonts) when this ships.
- **Skill doc:** N/A. No new agent-/API-drivable surface — persistence reuses the `prefs` verbs
  (already cataloged); looks/fonts/surfaces/motion are human-operated UI choices. If a later scope
  lets agents *set* a workspace look, that scope owns the skill.
