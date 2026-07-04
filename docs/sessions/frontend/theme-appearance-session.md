# Session — theme appearance (looks, fonts, surfaces, motion)

Status: **shipped**. Scope: [`scope/frontend/theme-appearance-scope.md`](../../scope/frontend/theme-appearance-scope.md).
Public: [`public/frontend/frontend.md`](../../public/frontend/frontend.md) (promoted on ship).

Successor to the shipped theme customizer. "10x the theme" — look packs, fonts, surfaces, a motion
system, wider tone palette, and the extension `ctx.theme` v4 fan-out. Plus slice 0: the two shipped
Customizer bugs (radius no-op, non-clickable brand swatches). Zero backend/Rust change — everything
rides the existing `ui_theme` prefs blob (client-side shape widen + normalize defaults).

Working directly on `master`, committing in reviewable slices.

---

## Slice 0a — radius bug (shipped, live-verified)

**Symptom (shipped).** The Customizer's radius control visibly did nothing for most of the app. Cards,
inputs, chips, buttons kept their corners no matter the slider.

**Root cause.** `theme-dom.ts` writes `--radius` correctly (unit-tested). But in `globals.css`'s
`@theme`, only `--radius-sm/md/lg` derived from `var(--radius)`; **bare `rounded`** (Tailwind
`--radius-DEFAULT` = static `0.25rem`), `rounded-xl` (static `0.75rem`), and `rounded-2xl` never
referenced the token — so ~114 `rounded` + `rounded-xl` call sites were pinned to Tailwind's compiled
defaults. **Second, deeper cause found during live-verify:** `@import "tw-animate-css"` re-imports
Tailwind's default `@theme`, which re-declares `--radius-sm/md/lg` **statically** *after* our `@theme`
block in the compiled sheet — so even deriving them in `@theme` lost the cascade to the static
`.375rem`. jsdom can't see any of this (no Tailwind, no `var()` computation), which is why it shipped.

**Fix (two parts).**
1. `globals.css` `@theme` now declares the full ladder `--radius-xs…--radius-3xl` (+ `--radius-DEFAULT`)
   off `var(--radius)`, so Tailwind *generates* `rounded-xl/2xl/3xl` utilities.
2. A **cascade-last `:root:root` override** (specificity 0,2,0) at the end of `globals.css` re-asserts
   the derived values so it beats `tw-animate-css`'s later plain-`:root` static re-emission by
   specificity, not source order (the bundler reorders past us). This is the load-bearing line.
   - sm/md/lg keep their shipped offsets (`--radius-2px`, `--radius`) so shadcn components are
     byte-identical — no visual regression. `rounded` (DEFAULT) is pinned to the `md` stop.
3. **Sweep:** all bare `rounded` → `rounded-md` across 44 `.tsx` files (perl word-boundary replace,
   `rounded-full`/`rounded-none` untouched). Makes the intended stop explicit and lets the guard forbid
   bare `rounded`.

**Live-verify (real Chromium, `@playwright/test`).** Loaded the compiled CSS, set `--radius` to
0.5rem / 1rem / 0rem, read `getComputedStyle().borderRadius`:

```
--radius=0.5rem : {"md":"6px","lg":"8px","xl":"12px","x2":"16px","full":"3.35e7px"}
--radius=1rem   : {"md":"14px","lg":"16px","xl":"20px","x2":"24px","full":"3.35e7px"}
--radius=0rem   : {"md":"0px","lg":"0px","xl":"4px","x2":"8px","full":"3.35e7px"}
```

Every stop tracks the token; `rounded-full` stays a pill; `0rem` clamps to square. Bug fixed.

**Guard test.** `src/styles/radius-scale.guard.test.ts` — asserts (a) every stop derives from
`var(--radius)` in source, (b) the `:root:root` cascade-last override exists, (c) no `.tsx` uses bare
`rounded`. Fails-before / passes-after; keeps the regression out.

**Debug entry:** [`debugging/frontend/radius-control-does-nothing.md`](../../debugging/frontend/radius-control-does-nothing.md).

Green: `radius-scale.guard.test.ts` 4/4; full unit suite 475/475 (was 472 + 3, then +1 override guard).

---

## Slice 0b — brand-color picker bug (shipped)

**Symptom (shipped).** In Brand Colors, only the tiny 24×32px swatch was clickable — the rest of the
row was dead. On the Tauri Linux desktop (WebKitGTK) the click did **nothing at all**: WebKitGTK ships
no native `<input type="color">`, so the whole control was a no-op there.

**Root cause.** `components/ui/color-picker.tsx` was a native `<input type="color">` wrapper. The hit
target was the OS swatch; the row wasn't a trigger; and the OS picker doesn't exist on WebKitGTK.

**Fix.** Replaced it in place with a hand-authored, token-bound **in-DOM popover** (no new dep, no
native input): whole labelled row is the `<button>` trigger (`aria-haspopup="dialog"`); the popover has
three H/S/L `type="range"` sliders + a hex text field; outside-click / Escape dismiss. Value math moved
to a new `lib/theme/hsl-triplet.ts` (`parseTriplet`/`formatTriplet`/`hslToHex`/`tripletToCss`) so it is
unit-testable without a DOM; hex input reuses the existing `colorToHslTriplet`. Same
`{label,value,onChange}` contract, so `BrandColors.tsx` is untouched.

**Tests.**
- `components/ui/color-picker.test.tsx` (6): no native `input[type=color]` in the DOM; row-click opens
  the dialog; a channel change emits a valid triplet; a hex value converts (`#ffffff`→`0 0% 100%`); an
  unparseable hex is ignored (fail-closed); Escape closes.
- `lib/theme/hsl-triplet.test.ts` (5): parse/format/clamp/wrap, HSL→hex primaries, CSS wrap.

Green: 487/487 unit (was 476 + 11 new).

---

## Slice 1 — tone widening + migration fold (shipped)

New tokens `--panel-2`/`--overlay`/`--accent-2` (derivable palette tones) + semantic
`--success`/`--warning` (fixed hues, like `--destructive`). `BASE_TOKENS` split into
`REQUIRED_TOKENS` (7) + `DERIVED_TOKENS` (3); `isBasePalette` demands only the required seven.

**Migration fold (the blocker).** `derive-tones.ts` derives the widened tones from the required
seven (raised surfaces step toward fg per mode; `accent2` rotates hue +40°). `normalize-custom-theme.ts`
fills absent tones on a stored theme; `normalizeThemePreference` runs it on `custom`/`imported`. A v1
seven-token theme survives + gains tones; a theme missing a *required* token still drops whole
(fail-closed). `theme-dom.ts` skips an absent tone so an un-normalized palette can't write `""`.

Tests: `derive-tones.test.ts` (3), `theme-options.test.ts` (4 — v1 survives, required-missing drops,
garbage→DEFAULT, already-widened preserved). 494/494. New tokens verified in compiled CSS.

**Decision:** `--success`/`--warning` are globals-only fixed tokens this slice (not member-editable),
per the scope's "define tokens + derivations now, sweep consumers opportunistically."

---

## Slices 2–5 — looks / fonts / surfaces / motion (shipped)

`ThemePreference` widened with `look` (required, default `default`) + optional `fontSans`/`fontMono`/
`surface`/`motion` (undefined = inherit the look). Everything rides the same `ui_theme` blob.

- **Looks** (`theme-looks.data.ts`, `look-resolve.ts`): six packs as DATA — default/editor/professional/
  retro/modern/glass, each a per-axis defaults bundle. `resolveAppearance` folds per-axis: **pinned look
  axis → explicit member override → look default → builtin**. Only `retro` pins (its preset, as data
  `pins:["preset"]`). `applyLook` resets the axes a look defines (drops overrides) so it lands like its
  thumbnail; `preset`/`radius` (required fields) are *stamped* by `applyLook`, not resolve-time defaulted.
  Added a `retro` phosphor-green preset (AA-vetted). **Decision:** looks DEFAULT the preset, `retro` pins
  — resolved in data, no branch (scope open-Q resolved).
- **Fonts** (`theme-fonts.data.ts`, `font-loader.ts`): Inter/Geist/IBM Plex Sans + Source Serif 4 (sans/
  serif), JetBrains Mono/IBM Plex Mono (mono); `--font-sans`/`--font-mono` in `@theme`, written by
  theme-dom. woff2 lazy-loaded (latin 400/600) via dynamic `import()` ONLY on selection; system stack is
  the zero-cost default. The dangling `var(--font-mono)` in JsonTree now resolves (token defined).
- **Surfaces** (`globals.css`): `data-surface` + tokens (`--surface-alpha`/`--blur`/`--shadow-1..3`/
  `--gradient-accent`); flat/elevated/glass restyle every `[data-panel]` (card/sheet/dialog tagged) by
  cascade. Glass→elevated→flat via `@supports` (never a platform branch).
- **Motion** (`lib/motion/`, `resolve-motion.ts`): `data-motion` CSS fence + `useMotionPref` JS seam;
  `motion` (motion.dev) imported in EXACTLY ONE file (`lib/motion/motion.ts`), enforced by a guard test.
  `prefers-reduced-motion` forces off unless the member chose `full`. **Decision:** default `subtle`.

**Live-verify (real Chromium):** `data-surface=elevated` → shadow ramp; `glass` → stronger shadow;
`flat` → none. `data-motion=off` → `transition-duration: 0s`; `subtle`/`full` → 0.3s kept. Fonts split
into their own lazy woff2 chunks (0 font refs in the main JS bundle).

Tests: look-resolve (6), derive-tones (3), resolve-motion (4), migration (4), AA contrast on all six
looks × both modes (6), theme-dom axis writes (8), font-loader + single-motion-seam bundle guard (2).

## Slice 6 — ctx.theme v4 fan-out (shipped)

- `theme-events.ts`: the shell-internal `lb:themechange` pub/sub; theme-dom emits once per application.
- `resolve-theme-tokens.ts`: the WIDENED `ctx.theme` (base + tones + radius + fonts + surface + motion +
  core chart ramp) from `getComputedStyle` (honors custom/imported/inline) + the resolved appearance.
- `useThemeTokens.ts`: re-resolves on each emit (`useSyncExternalStore`); degrades to `DEFAULT_THEME`
  outside a `ThemeProvider` (`useThemeOptional`) so a standalone ext widget doesn't crash.
- `ExtWidget` threads `ctx.theme` into the memoized ctx (keyed on `themeKey`) → the existing `update(ctx)`
  path recolors the widget in place, no re-mount.
- **WIDGET_CTX_V 3→4, additive, all three mirrors** (host `federationWidget.ts` + `ExtWidget.tsx`, devkit
  template, echarts + thecrew copies). echarts-panel is the reference consumer: `framesToOption` recolors
  series/axis/text from `ctx.theme`. A lockstep guard test pins all mirrors at v4.

Tests: theme-events (3), resolve-theme-tokens (2), ctx.theme forwarding (+1), live re-theme via update
(1, mount-once), contract-mirror lockstep (5). **Gateway (real node, mandatory):** widened blob
round-trip + workspace-default fold + capability deny (no `prefs.set`; non-admin `set_default`) +
workspace isolation + reset — `theme-prefs.gateway.test.ts` 6/6.

## Slice 7 — the VISIBLE payoff: consume the multi-tone (shipped, live-verified)

The plumbing shipped in slices 1–6 defined `--panel-2`/`--overlay`/`--accent-2`/`--success`/`--warning`
but **nothing consumed them** — the shell still rendered 2-tone. This slice sweeps the real consumers so
the eye sees >2 tones:

- **Raised surfaces → `--panel-2`.** The nav rail (`sidebar.tsx`, all three render paths), page-header
  band (`.page-header` in `globals.css`), tab bars (`TabsList`), and the shared left column
  (`components/app/rail.tsx` — used by the dashboard roster and every rail surface) now sit a step above
  the page `--bg`. The rail + cards are also tagged `data-panel` so the look's Surface treatment
  (elevated shadow / glass blur) applies to them by cascade.
- **Overlays/scrims → `--overlay`.** `--overlay`'s derived semantic was retargeted to a real **scrim**
  (near-black in BOTH modes — a modal backdrop must darken content regardless of mode). The dialog/sheet
  backdrops (`bg-black/50` → `bg-overlay/60`) consume it. `deriveTones` + the static blocks + the
  contrast fixtures updated in lockstep; `derive-tones.test.ts` gained a scrim assertion.
- **Secondary/ghost accents → `--accent-2`.** The active-nav pill (`sidebarMenuButtonVariants`) now reads
  as its own tone (`bg-accent-2/15 text-accent-2`) instead of a third grey; dashboard-cell hover borders
  use `accent-2`. New `Badge` variants `success`/`warning`/`accent2` give the semantic tokens real,
  opt-in consumers.
- **Semantic status → `--success`/`--warning`.** Telemetry level/outcome badges (`TelemetryList`) route
  through `success`/`warning`/`destructive` tokens instead of hardcoded Tailwind palette colors, so they
  re-theme with the look.

**Live-verify:** every raised surface reads a step off the page bg; glass backdrop-filter now fires on
the tagged surfaces (was reading `none` because the first `[data-panel]` probed wasn't in scope — the
real cause was untagged surfaces, not a broken `@supports`).

## Slice 8 — the looks read as THEMSELVES (shipped, live-verified)

The looks were near-identical muddy charcoal. Re-authored the preset palettes (`theme-presets.data.ts`,
in oklch for perceptual control) AND the look bundles so each look is distinct:

- **`mode` is now a look axis.** A look stamps light/dark on pick (`LookDefaults.mode`, stamped in
  `applyLook` like preset/radius; the member can still flip after). This is what makes **Professional**
  genuinely *light paper* and **Modern** an *airy light dashboard* — previously they stayed dark because
  mode was independent of the look. (Decision: add mode as a look axis — user-confirmed.)
- **Distinct palettes:** default = warm charcoal + amber (dark); **editor** = new cool slate-blue
  near-black + cyan syntax accent (dark, sharp); **professional** = white paper + indigo + serif (light,
  elevated); **retro** = phosphor green on near-black + mono (dark, square); **modern** = airy
  cyan-tinted white + large radius (light, elevated); **glass** = violet-plum + translucent blur (dark).
  Added a new `editor` preset (its own cool-slate identity, distinct from `slate`).
- **AA re-vetted:** `contrast.test.ts` iterates every look's preset × both modes — green (fg/bg 10–16,
  accent/bg ≥4.3).

## Slice 9 — motion actually animates (shipped, live-verified)

`lib/motion` (the motion.dev seam) + `useMotionPref` + `data-motion` were wired but **nothing animated**,
so Off/Subtle/Full were inert. Built the seam's primitives (motion.dev imported ONLY in `motion.ts` —
guard test still green) and wired the real surfaces the scope named:

- **`Reveal`** (fade+slide, gated) → page-body mounts (`components/app/page.tsx`, keyed on surface) and
  settings **tab transitions** (`TabsContent`, keyed on value).
- **`Stagger`/`StaggerItem`** → the **look-card grid** (staggered mount).
- **`Collapse`** (height animation) → the **Brand-colors accordion** (`accordion.tsx`).
- **CSS `data-motion` tuning** → the **nav-rail collapse** (subtle 120ms / full 320ms spring-ease / off
  0s) — the same fence that zeroes transitions when off now *scales* subtle-vs-full.
- Every primitive renders **statically when off** (or under `prefers-reduced-motion` — `resolveMotion`
  folds subtle→off). `motion-gate.test.tsx` (4) proves the off switch.

**Live-verify (real Chromium):** `data-motion` tracks the picker (off/subtle/full); the rail transition
measured 0s / 0.12s / 0.32s+spring-ease respectively; the accordion Collapse and page/tab Reveals fire;
no console errors.

## Dashboard focus — multi-tone + glass on the board (shipped, live-verified)

Follow-up after live review: the sweep landed clearly on the sidebar but the **dashboard** still read
2-tone and **glass looked flat on the board**. Root cause: the dashboard panel cells (`Grid.tsx`) and the
roster column weren't tagged `data-panel`, so the Surface treatment never reached them.

- **Grid cells** (`Grid.tsx`): tagged `data-panel`, shadow bound to `var(--shadow-1/2)`, hover border
  `accent-2` — so flat/elevated/glass now read on each widget panel.
- **Roster/rail** (`components/app/rail.tsx`): `bg-panel` → `bg-panel-2` + `data-panel`, giving the board
  a real **3-tone hierarchy** (recessed grid `--bg` → panel cells `--panel` → raised rail `--panel-2`).
  Shared `AppRail` means every rail surface gains the third tone consistently.
- **Dashboard toolbar** strip → `bg-panel-2/70`.

**Live-verify (real board, Ops Overview):** cell computed style — glass = `rgba(...,0.72)` +
`blur(14px)` + shadow; elevated = opaque + shadow ramp; flat = opaque, no shadow. The 3 tones are
visible in the screenshot (rail/roster raised, grid recessed, cells floating).

## Dashboard design polish — impeccable pass (shipped, live-verified)

Live review flagged the dashboard as still flat/muddy despite the multi-tone tagging, and the glass look as
fighting the product brief. Ran the **impeccable** skill (product register). Its `PRODUCT.md` anti-refs are
explicit: *avoid fake glass panels, dashboard theatrics, terminal cosplay*; the fix is **crisp hierarchy +
legible contrast**, not more effects.

- **Dark ramp retuned for VISIBLE separation.** The 3–4% lightness steps were invisible. New dark ramp:
  `--bg 7%` (true near-black) → `--panel 11%` (cells) → `--panel-2 15%` **nudged cooler** (hue 24→230, low
  chroma) so the raised neutral layer reads as its own surface (product register: "a second neutral layer,
  slightly cooler"). `--border` lifted 18%→22% so hairlines actually register on dark. AA re-vetted (the
  amber fixture in `contrast.test.ts` updated in lockstep — green).
- **Cells carry elevation by BORDER + inset highlight, not shadow.** `Grid.tsx` cells: `rounded-xl`, crisp
  `border-border`, a 1px `inset 0 1px 0 hsl(--fg/0.04)` top-highlight (the Linear/Stripe "lifted" trick that
  reads far better than a mushy drop-shadow on dark), hover brightens the border toward `accent-2`. Edit
  handles (`move`/`remove`) now **reveal on hover/focus** (quiet resting state) instead of always-on clutter.
- **Designed in-cell placeholder.** New `WidgetPlaceholder.tsx` replaces the bare "unsupported widget" /
  "panel not accessible" muted-text lines with an honest state (icon tile + title + one-line detail; `warn`
  tone for unsupported). Product register: "empty states that teach, not 'nothing here'".
- **Empty state upgraded.** `AppEmptyState` dropped the dashed border for a solid bordered card with the same
  inset-highlight elevation + an accent-tinted icon tile (used across Dashboards/Flows/etc).
- **Rail is the raised neutral layer** (`components/app/rail.tsx`): `bg-panel-2` + `data-panel`, giving every
  rail surface app-wide the third tone consistently.

**Live-verify (real board, dark/flat default):** the unsupported-widget cell now reads as a designed panel;
rail/roster/grid/cell form a legible depth hierarchy; the empty state reads premium. Screenshots in scratch.
**Note on glass:** kept as an opt-in *look* (the scope requires it) but it is NOT the default — the default
board is crisp/flat per the product anti-references.

## Green output

- `pnpm test`: **532/532** unit (was 527; +4 `motion-gate`, +1 `derive-tones` split).
- `pnpm test:gateway`: `theme-prefs.gateway.test.ts` **6/6** on a real spawned node. Pre-existing reds
  untouched and confirmed not ours: `sqlSource.gateway` (casing), `ProofPanel.gateway` (missing-WASM
  fixture), `SystemView.gateway` (concurrent CodeEditor edit), `InboxView.gateway` (duplicate
  `needs:approval` items — fails identically at the pre-work base commit's own test content).
- `pnpm build`: my code + CSS + fonts compile (`vite build` green; font/motion chunks split). `tsc
  --noEmit` in the build script fails only on **3 pre-existing** gateway-test type errors
  (`FlowsCanvas.gateway.test.ts` ×2, `transformDebug.gateway.test.tsx` ×1) present on clean master —
  not this scope's, left untouched.
- `pnpm lint`: **0 new errors** in any new/edited file; the 8 raw-`<button>` errors are the pre-existing
  baseline (all in unrelated features; my Grid.tsx edit only touched existing raw-button *warnings*).

## Decisions recorded (scope open questions)

- **`mode` is a look axis** (slice 8) — a look stamps light/dark on pick so Professional/Modern land as
  *light* looks; member can flip after. Resolved in data (`LookDefaults.mode`), no branch. User-confirmed.
- **`--overlay` is a scrim** (slice 7) — retargeted from a raised-bg tone to a near-black scrim in both
  modes, so modal backdrops darken regardless of mode. Its only consumers are dialog/sheet backdrops.
- **Multi-tone lands on shared primitives** (`sidebar`, `.page-header`, `TabsList`, `AppRail`, `Badge`,
  dashboard `Grid`) so one edit re-tones many surfaces — not an app-wide per-file sweep.

- **Font list:** Inter/Geist/IBM Plex Sans + Source Serif 4 (professional); JetBrains Mono/IBM Plex Mono;
  retro uses JetBrains Mono — no novelty/pixel face (retro is palette + shape). @fontsource OFL,
  self-hosted, latin subset, lazy on selection.
- **Looks default the preset; `retro` pins it** — data `pins:["preset"]`, no branch.
- **`--success`/`--warning`:** tokens + derivations defined now; consumers swept opportunistically.
- **Density axis:** DEFERRED (noted in the scope open questions).
- **Iframe tier:** NOT built; the emitter is designed so the iframe fan-out is just another subscriber.
- **Motion default:** `subtle`; reduced-motion forces off unless explicit `full`.
- **Stored-theme migration:** required-vs-derivable split; a v1 custom palette survives (derivation),
  never a fail-closed drop.
