# Theme palette research — the five weak looks

**Status:** APPLIED (`theme-presets.data.ts`). All 63 theme tests + typecheck green.
The critical follow-up after applying: the two LIGHT looks (Professional + Modern) still read alike
because "white page + faint cards + dark text" is the same *structure* regardless of accent hue. The
fix was structural, not chromatic — see "Light-look separation" below.
**Date:** 2026-07-04
**Trigger:** "these colours ARE so bad — retro, code editor, liquid glass, modern dashboard,
professional. Only operator console looks good. Research and find the top palettes; make sure they
look good in dark AND light."

All values are OKLCH. Every proposed pair was run through a WCAG contrast checker
(`scratchpad/run.mjs`) — **every fg/muted pair clears 4.5:1 and every accent clears 3:1, in both
modes** (numbers per look below). The presets live in
`ui/src/lib/theme/theme-presets.data.ts` (the adapter reads bg/fg/card/muted/muted-foreground/
primary/accent/border and derives the rest).

---

## Root-cause diagnosis (from real renders, not code)

I screenshotted each look on the System page. Two failures repeat:

1. **The neutral ground is the same everywhere.** Editor, Modern, and Professional all sit on a
   near-identical blue-tinted white (light) or blue-black (dark) with a low-chroma accent. Change the
   accent, keep the ground → every look reads as *"stock shadcn, different accent."* That's exactly
   the "99% the same" complaint. The fix is not a brighter accent; it's giving **each look its own
   neutral temperature** so the *whole surface*, not just one button, says which look you're in.

2. **Retro's neutrals carry the accent's chroma.** Today Retro's bg/card/muted are all green
   (`#0a0f0a`, `muted #5f9e72`). When the ground, the borders, the muted text, AND the accent are all
   the same hue, nothing separates from anything — it reads as green mud, not a terminal. Real
   terminals put a **saturated phosphor accent on a near-neutral (near-zero-chroma) black.** The
   contrast between a colored accent and a colorless ground is the whole aesthetic.

Operator Console works precisely because it already does #1 right: a cool near-black ground (hue 228)
with a warm amber accent (hue 34). The ground and the accent disagree on hue, so the accent reads as
a deliberate signal. Every fix below applies that same principle with a different hue pairing.

---

## The five replacement palettes

Each is authored so **dark and light are the same identity**, not two unrelated themes — same accent
hue family, mode-appropriate lightness/chroma (brighter+more chroma on dark, deeper on light).

### 1. Code Editor — "Tokyo Night" character

The reference dark-IDE look: a deep indigo-slate ground (not the current flat blue-black), one
electric-blue accent, cool desaturated chrome. Distinct from Operator Console by being cooler and
indigo-forward instead of amber.

| token  | dark               | light              |
|--------|--------------------|--------------------|
| bg     | `0.20 0.02 265`    | `0.99 0.003 255`   |
| card   | `0.245 0.025 265`  | `0.975 0.004 255`  |
| fg     | `0.90 0.02 255`    | `0.27 0.03 262`    |
| muted  | `0.65 0.03 258`    | `0.47 0.03 258`    |
| accent | `0.72 0.15 235`    | `0.52 0.17 258`    |

Contrast — dark: fg 13.5, muted 5.6, accent 7.5. light: fg 14.7, muted 6.6, accent 5.5.

### 2. Retro Terminal — amber phosphor on true black

The big change: **neutrals drop to near-zero chroma** so the amber pops. Amber phosphor (not green)
reads as "CRT terminal" while separating cleanly from the green success dots. Light mode is a warm
paper/sepia terminal for parity.

| token  | dark               | light              |
|--------|--------------------|--------------------|
| bg     | `0.145 0.008 75`   | `0.97 0.010 85`    |
| card   | `0.18 0.012 75`    | `0.945 0.014 85`   |
| fg     | `0.85 0.13 80`     | `0.28 0.04 60`     |
| muted  | `0.60 0.09 82`     | `0.46 0.05 65`     |
| accent | `0.80 0.165 72`    | `0.52 0.13 55`     |

Contrast — dark: fg 12.4, muted 5.0, accent 10.3. light: fg 13.5, muted 6.6, accent 5.3.
(If you prefer to keep Retro **green**, swap hue 72–85 → 140 at the same L/C — the point is the
zero-chroma ground, not the hue.)

### 3. Modern Dashboard — airy, sky-cyan

Its identity is *bright and open*, so the win is a genuinely clean near-white ground with a
confident cyan-sky accent (shifted off Glass's plum so the two light looks don't collide).

| token  | light              | dark               |
|--------|--------------------|--------------------|
| bg     | `0.995 0.005 240`  | `0.205 0.02 255`   |
| card   | `0.98 0.008 235`   | `0.25 0.025 255`   |
| fg     | `0.27 0.03 250`    | `0.93 0.015 250`   |
| muted  | `0.49 0.03 245`    | `0.66 0.03 250`    |
| accent | `0.55 0.16 245`    | `0.74 0.14 240`    |

Contrast — light: fg 14.8, muted 6.1, accent 4.7. dark: fg 14.6, muted 5.8, accent 7.9.

### 4. Professional — paper + a teal-slate ink

Light-forward serif look. The move is a **true off-white** (chroma near 0, not blue-tinted) with a
restrained deep-teal accent — reads as ink on paper, the "serious document" voice the look promises.

| token  | light              | dark               |
|--------|--------------------|--------------------|
| bg     | `0.995 0.002 230`  | `0.19 0.012 240`   |
| card   | `0.975 0.004 220`  | `0.235 0.015 240`  |
| fg     | `0.25 0.02 240`    | `0.92 0.01 235`    |
| muted  | `0.46 0.02 235`    | `0.64 0.02 238`    |
| accent | `0.48 0.10 205`    | `0.72 0.11 200`    |

Contrast — light: fg 15.7, muted 7.0, accent 6.0. dark: fg 14.6, muted 5.5, accent 7.8.

### 5. Liquid Glass — deep indigo-plum ground

Glass only reads if the ground has enough chroma to *tint the blur*. Raise the dark ground's chroma
(`0.18 0.04 290`) so panels refract violet, and give it a luminous lavender accent. Light is a soft
lilac paper.

| token  | dark               | light              |
|--------|--------------------|--------------------|
| bg     | `0.18 0.04 290`    | `0.99 0.006 300`   |
| card   | `0.235 0.055 290`  | `0.965 0.012 300`  |
| fg     | `0.93 0.02 290`    | `0.27 0.04 300`    |
| muted  | `0.66 0.04 290`    | `0.47 0.04 300`    |
| accent | `0.74 0.16 300`    | `0.52 0.19 298`    |

Contrast — dark: fg 15.3, muted 6.0, accent 7.7. light: fg 14.8, muted 6.7, accent 5.9.

---

## Why these five separate from each other (the anti-"all the same" check)

Read the accent hues together: amber-warm (Operator 34), electric-indigo (Editor 235), amber-phosphor
(Retro 72), sky-cyan (Modern 245), teal-slate (Professional 205), lavender-plum (Glass 300). Six
looks, six distinct hue families — and, more importantly, six distinct *ground temperatures*. You can
tell which look you're in from the empty background alone, which is the bar Operator Console already
met and the others missed.

## Light-look separation (the "they're all the same" fix)

Accent hue alone does NOT distinguish two light looks — the eye reads the ground, and two near-white
grounds look identical no matter the button color. The looks are separated by STRUCTURE + temperature:

- **Professional** = warm ivory ground (hue 85, low chroma) + **plain white cards** (card lighter than
  bg, flat) + teal ink. Reads as ink on paper.
- **Modern** = cool blue-tinted **canvas** (`0.965 0.018 240`, visibly not white) + **white cards that
  lift off it** (card is lighter than bg → the airy-dashboard "floating cards" signature) + sky-blue.

The warm-vs-cool ground and the flat-vs-floating card relationship are what make them read as two
different products at a glance — the accent hue is secondary. Final applied values are in
`theme-presets.data.ts` (Professional light, Modern/ocean light).

## Applying this (when approved)

Each look pins a preset (`theme-looks.data.ts`): editor→editor, professional→slate, retro→retro,
modern→ocean, glass→violet. Swap the `light`/`dark` blocks of those five preset entries in
`theme-presets.data.ts` with the values above (converting the seven adapter-read tokens; the widened
tones re-derive). No component or adapter change needed — it's a data swap, and
`preset-adapter.test.ts` guards the round-trip. Recommend applying one look at a time with a
screenshot check in both modes.

**Evidence:** `scratchpad/run.mjs` (contrast proof, all pass), `scratchpad/palettes.png` (swatch +
mini-card preview of every palette), `scratchpad/L/*.png` (current-state renders that motivated each
change).
