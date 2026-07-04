# Session — Liquid Glass ambient field + categorical color sweep

**Date:** 2026-07-04
**Area:** frontend (theme surfaces + System page + chart palette)
**Ask (verbal):** "Theme settings are set up but the app still looks bad — Liquid Glass reads as 2–3
flat colours. Focus on `/dashboards` and `/system`; make the brand colours actually get used. 10x the
look."

## Diagnosis (from live screenshots, Playwright against the dev node)

1. **Glass had nothing to refract.** Every surface (page, panels, sidebar) sat on the same flat
   near-black violet. `backdrop-filter` on the `[data-panel]` cascade was already wired
   (theme-appearance scope) but blurring a flat ground is a no-op — the whole look collapsed into a
   monochrome soup.
2. **The widened palette never reached the UI.** `--accent-2`, `--success`, `--warning` existed but
   the System grid hardcoded `emerald-500`/`amber-500`, and both chart palettes
   (`charts/chartTheme.ts`, `dashboard/views/field.ts`) were literal `hsl()` strings — so no preset,
   mode, or theme edit could ever re-voice them, and the pages read as 2–3 colours.
3. **The System grid was nine identical cards** — title + blurb + grey chips; the only colour on the
   page was the green Ok dot.

## What shipped

- **`ui/src/styles/globals.css`**
  - `--chart-1…8` categorical ramp, tuned per mode (deeper cuts on paper, brighter on near-black),
    exposed to Tailwind via `@theme` as `--color-chart-N`. Identity colour only — state stays
    success/warning/destructive/accent.
  - **The ambient field**: under `[data-surface="glass"]` (inside the existing `@supports` gate),
    `body` + every `.bg-bg` element paint two viewport-anchored brand glows (accent top-right,
    `--accent-2` bottom-left) with `background-attachment: fixed`, so all elements show slices of ONE
    continuous field and translucent panels finally have something real to blur. Glow alpha scales
    with `data-glass` (subtle/medium/heavy) via `--glow-1/--glow-2`; light mode halves it
    (`--glow-scale: 0.55`) because a wash muddies paper sooner than near-black.
  - **Glass edge**: `[data-panel]` under glass gets a light inner rim
    (`inset 0 1px 0 hsl(var(--fg)/0.08)`) + `border-color: hsl(var(--fg)/0.13)` — the physical
    "pane" cue; without it translucency reads as smudge.
- **`ui/src/features/charts/chartTheme.ts`** + **`ui/src/features/dashboard/views/field.ts`** —
  both categorical palettes now read `hsl(var(--chart-N))` (one mode-tuned ramp, theme-followable;
  the two files previously carried two different hardcoded ramps that drifted).
- **`ui/src/features/system/identity.ts`** (new) — the one subsystem→(icon, `--chart-N` hue) map so
  the grid/graph/sheet can colour a subsystem identically. Core subsystem ids only (no rule-10
  concern).
- **`ui/src/features/system/SystemView.tsx`** — StatusCard gets a hue-tinted icon tile (identity,
  not state), description clamped to 2 lines, drill-affordance glyph reveals on hover, metric chips
  render values in mono/tabular. Health dot/pill unchanged in structure.
- **`ui/src/features/system/health.ts`** — ok/degraded now use the semantic `--success`/`--warning`
  tokens instead of raw `emerald-500`/`amber-500` (down stays `destructive`, idle stays muted).

**Rejected alternative:** painting the glow only on `body` — opaque `bg-bg` page roots (SystemView,
Grid canvas) would have covered it; the `.bg-bg`-with-fixed-attachment cascade is what makes the
field show through everywhere without touching a single component.

## Verification

- Live Playwright screenshots (dark + light glass, subtle/medium) of `/t/acme/system` and
  `/t/acme/dashboards` with the E2E board open — glass panes now visibly refract the field; System
  cards carry 8 distinct identity hues; charts pick up the token ramp.
- `pnpm vitest run src/features/system src/features/charts src/features/dashboard/views src/lib/theme`
  → **20 files, 97 tests, all green**.
- `tsc --noEmit` → no errors in touched files (3 pre-existing errors in
  `FlowsCanvas.gateway.test.ts` / `transformDebug.gateway.test.tsx`, present on a clean tree).
- Flat/elevated looks untouched (all new CSS is gated on `data-surface="glass"`); the no-blur
  fallback ladder is unchanged.

## Wave 2 — shell chrome (user: "still looks stock shadcn; give me a real header/footer")

The token work was real but invisible at a glance; the chrome is what the eye reads. Shipped:

- **`components/app/page-header.tsx`** — the header now owns its band: an accent wash rising from the
  title side, a two-hue signature hairline (accent → accent-2 → neutral) replacing the flat
  `border-b`, a gradient icon tile, a 15px tracking-tight title. The workspace chip restyled to the
  StatusBar voice (mono + accent dot), and **Settings moved here** — a gear at the top-right (plain
  hash link `#/t/<ws>/settings`, so the component stays router-free; route gates re-check caps).
- **`features/shell/StatusBar.tsx`** (new) + mounted in `RoutedShell` — a 28px ops strip under every
  page: workspace wall + principal (left), cap count + active look (right). Honest session/theme
  facts only, no polling, no invented "connected" dot.
- **`features/shell/NavRail.tsx`** — brand mark is now the signature accent→accent-2 gradient tile
  (`--accent-foreground` glyph); Settings removed from the rail footer (moved to the header gear;
  a server-authored nav can still place it). `NavRail.test.tsx` updated to assert the removal.
- **`components/ui/sidebar.tsx`** — active nav pill gains a 1px inset accent ring.
- **`features/system/SystemView.tsx`** — `HealthStrip`: the one-line verdict ("All subsystems
  operational." / "N want attention") + per-health counts as chips, replacing the old microcopy line.

Verification wave 2: full unit suite **94 files / 561 tests green**; `tsc --noEmit` clean for touched
files; screenshots across dark glass, light glass, flat Operator Console, and light Professional.

## Follow-ups (not done)

- Colour `SystemTopologyGraph` nodes with the same `identity.ts` hues (grid/graph parity).
- The Settings→Theme Look cards could preview their look's actual palette swatches.
