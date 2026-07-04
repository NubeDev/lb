# Scope — App design system (Unistyles 3 + UI kit)

**Status:** first slice shipped (theme + kit + login/channels re-skin, verified in the web preview).

## The ask

The RN app (`app/shell`) had functional-but-plain screens (grey `Button`s, light default
chrome). Give it a real design system so it "looks amazing and is fast": the dark, mint-accent
smart-home look (near-black canvas, softly-raised hairline-bordered cards, one saturated accent used
sparingly, dotted-ring gauges, springy toggles). The user's reference set was dark home-automation /
energy dashboards (Xiaomi / MYGRID style).

## Decision: react-native-unistyles 3 (not NativeWind)

Unistyles 3 is a C++/Nitro engine — theme and breakpoint updates hit the Shadow Tree directly with
**no React re-render**, so it's the fastest RN styling layer and beats a Tailwind-in-RN setup on the
"fast" requirement. It also gives first-class typed themes/variants, which is the shadcn-style
token workflow the user already knows. **Rejected:** NativeWind v4 + react-native-reusables (very
familiar, but a slower styling layer, and pulls in Tailwind tooling the app doesn't otherwise need).

Components: **own them** (the shadcn philosophy), not a styled RN kit. First four primitives built in
`shell/src/ui/` (Card, Tile, Toggle, GaugeRing). `@rn-primitives` is the recommended source for
future behavior-heavy, accessible primitives (dialog/dropdown/tabs) to skin with Unistyles.

## What shipped

- **Tokens** `shell/src/theme/tokens.ts` — one `dark` theme: ink/surface/line/mint palette, radii,
  a `space(n)` 4pt grid, font weights. Registered once in `theme/unistyles.ts` (imported at the top
  of `index.js` and `web/index.web.tsx`). Types augmented in `theme/theme-augment.d.ts`.
- **Nav theme** `theme/navigation.ts` — React Navigation `DarkTheme` mapped to our tokens so stack
  chrome matches.
- **UI kit** `shell/src/ui/` — `Card`, `Tile`, `Toggle` (Animated spring, no extra native dep),
  `GaugeRing` (react-native-svg dotted ring).
- **Re-skin** — `LoginScreen` and `ChannelsScreen` moved to the kit + tokens (logic untouched).

## Build wiring (Unistyles is a Babel-transform lib)

- **Native (Re.Pack/rspack):** official `RepackUnistylePlugin` from
  `react-native-unistyles/repack-plugin` in `rspack.config.mjs`.
- **Web preview (Vite):** `vite-plugin-react-native-web` transpiles via Rolldown/esbuild (no Babel),
  so a custom `web/unistyles-babel.vite.ts` runs the `react-native-unistyles/plugin` Babel pass over
  our own `src/`+`web/` sources only. react-native-svg's Fabric internals pull native-only RN modules
  RN-Web lacks → aliased to a DOM-`<svg>` shim `web/svg-shim/` for the preview only (device uses the
  real package). This is a preview rendering shim, not a fake backend (rule 9 respected — no node
  behavior reimplemented).

## Verification

`pnpm typecheck` green. Web preview (`vite.config.web.mts`) screenshotted via puppeteer: login screen
renders the dark/mint look; a throwaway kit gallery confirmed Card/Tile/Toggle/GaugeRing (60 gauge
dots drawn through the svg shim). Screenshots in the session log.

## Open questions / next

- Bundle the display font (Space Grotesk / General Sans) + Inter — tokens currently fall back to
  `System`. Needs an asset step (native font linking + web `@font-face`).
- Add `@rn-primitives` for accessible dialog/dropdown/tabs as screens need them.
- Consider `@shopify/react-native-skia` for the glow/waveform effects in the reference voice screen
  (heavier; only if a screen needs it).
- A device (Android) screenshot to confirm the native `RepackUnistylePlugin` path — only the web
  preview path is visually verified so far.
