// The built-in preset LIBRARY — DATA, not code (FILE-LAYOUT: presets are data files, never branches).
// A curated, contrast-vetted subset of the shadcn/tweakcn packs (the scope's recommendation: ship a
// vetted subset for the library, keep Import for the long tail). Each entry is a `ThemePreset` in the
// shadcn vocabulary; the adapter maps it onto base tokens at apply-time. Adding a preset = adding a row
// here. The three BUILTIN accents (amber/teal/blue) are NOT here — they apply via `data-theme-accent`
// and live in `globals.css`; this file is the *library* beyond them.
//
// The adapter reads ONLY: background, foreground, card, popover, primary, muted, muted-foreground,
// accent, border. The widened tones (panel-2/overlay/accent-2) DERIVE from those. Each preset below is
// authored as a deliberate design — a distinct hue family and lightness character so the six look packs
// never read as charcoal clones. Values are oklch (perceptual control) for the fully re-authored packs.

import type { PresetEntry } from "./theme-preset";

export const THEME_PRESETS: readonly PresetEntry[] = [
  {
    // The flagship LIGHT look (used by the `professional` look). Light = genuine PAPER: a clean cool
    // near-white, deep navy ink, a restrained deep-teal accent, soft cool-grey borders and a real text
    // hierarchy. Dark is a deep navy night mode of the same voice. The raised card sits a touch off the
    // page so panels read.
    value: "slate",
    name: "Slate",
    preset: {
      label: "Slate",
      styles: {
        light: {
          // Professional = CLEAN PAPER with deep-navy INK: a barely-cool near-white ground, white cards
          // that read through elevation (the look's surface is `elevated`), a deep navy text voice and
          // a restrained deep-teal accent. Reads as ink on a serious document; the navy ink (not a warm
          // tint) is what separates it at a glance from Modern's visibly blue-washed canvas.
          background: "oklch(0.985 0.004 230)",
          foreground: "oklch(0.29 0.045 258)",
          card: "oklch(1 0 0)",
          popover: "oklch(1 0 0)",
          primary: "oklch(0.47 0.09 192)",
          muted: "oklch(0.5 0.02 245)",
          "muted-foreground": "oklch(0.43 0.02 245)",
          accent: "oklch(0.47 0.09 192)",
          border: "oklch(0.89 0.012 235)",
        },
        dark: {
          // Dark = the same document at night: a deep navy ground (clearly blue, not charcoal), navy
          // panels a step up, and the teal accent brightened to carry on the dark ground.
          background: "oklch(0.20 0.03 252)",
          foreground: "oklch(0.93 0.012 235)",
          card: "oklch(0.245 0.035 252)",
          popover: "oklch(0.245 0.035 252)",
          primary: "oklch(0.75 0.11 190)",
          muted: "oklch(0.64 0.025 245)",
          "muted-foreground": "oklch(0.73 0.025 245)",
          accent: "oklch(0.75 0.11 190)",
          border: "oklch(0.33 0.03 250)",
        },
      },
    },
  },
  {
    // Editor — the DARK code surface (pinned by the `editor` look). A near-black slate-blue page (VS Code
    // Dark+ character), cool desaturated chrome, ONE cyan-teal syntax accent, and a "comment-grey"
    // secondary tier. Everything low-chroma except the accent; distinct from Slate by hue (cyan not
    // indigo) and by its deeper, cooler, flatter ground. Light is a crisp editor-paper for parity.
    value: "editor",
    name: "Code Editor",
    preset: {
      label: "Code Editor",
      styles: {
        light: {
          background: "oklch(0.99 0.003 255)",
          foreground: "oklch(0.27 0.03 262)",
          card: "oklch(0.975 0.004 255)",
          popover: "oklch(0.975 0.004 255)",
          primary: "oklch(0.52 0.17 258)",
          muted: "oklch(0.55 0.03 258)",
          "muted-foreground": "oklch(0.47 0.03 258)",
          accent: "oklch(0.52 0.17 258)",
          border: "oklch(0.9 0.006 258)",
        },
        dark: {
          // Editor = Tokyo-Night: a deep indigo-slate ground (distinctly bluer/deeper than Operator's
          // neutral charcoal) with an electric indigo-blue accent.
          background: "oklch(0.20 0.02 265)",
          foreground: "oklch(0.90 0.02 255)",
          card: "oklch(0.245 0.025 265)",
          popover: "oklch(0.245 0.025 265)",
          primary: "oklch(0.72 0.15 235)",
          muted: "oklch(0.65 0.03 258)",
          "muted-foreground": "oklch(0.72 0.03 258)",
          accent: "oklch(0.72 0.15 235)",
          border: "oklch(0.31 0.025 262)",
        },
      },
    },
  },
  {
    // Emerald — a clean botanical green (library preset, no look). Light = white with a faint green leaf
    // card; dark = deep forest. Distinct from Ocean/Editor by its warm-leaning green hue.
    value: "emerald",
    name: "Emerald",
    preset: {
      label: "Emerald",
      styles: {
        light: {
          background: "oklch(0.995 0.004 150)",
          foreground: "oklch(0.26 0.03 158)",
          card: "oklch(0.97 0.012 150)",
          popover: "oklch(0.97 0.012 150)",
          primary: "oklch(0.55 0.13 158)",
          muted: "oklch(0.5 0.03 158)",
          "muted-foreground": "oklch(0.42 0.03 158)",
          accent: "oklch(0.55 0.13 158)",
          border: "oklch(0.9 0.02 152)",
        },
        dark: {
          background: "oklch(0.2 0.025 158)",
          foreground: "oklch(0.94 0.015 152)",
          card: "oklch(0.25 0.03 158)",
          popover: "oklch(0.25 0.03 158)",
          primary: "oklch(0.78 0.15 160)",
          muted: "oklch(0.62 0.03 156)",
          "muted-foreground": "oklch(0.72 0.03 154)",
          accent: "oklch(0.78 0.15 160)",
          border: "oklch(0.32 0.03 158)",
        },
      },
    },
  },
  {
    // Rose — a warm crimson-pink (library preset, no look). Light = blush paper, dark = deep wine.
    // Distinct by its red-pink hue and warm neutrals.
    value: "rose",
    name: "Rose",
    preset: {
      label: "Rose",
      styles: {
        light: {
          background: "oklch(0.99 0.006 12)",
          foreground: "oklch(0.28 0.05 8)",
          card: "oklch(0.965 0.014 8)",
          popover: "oklch(0.965 0.014 8)",
          primary: "oklch(0.56 0.2 14)",
          muted: "oklch(0.52 0.04 8)",
          "muted-foreground": "oklch(0.44 0.04 8)",
          accent: "oklch(0.56 0.2 14)",
          border: "oklch(0.9 0.02 8)",
        },
        dark: {
          background: "oklch(0.2 0.03 8)",
          foreground: "oklch(0.94 0.015 8)",
          card: "oklch(0.25 0.035 8)",
          popover: "oklch(0.25 0.035 8)",
          primary: "oklch(0.75 0.16 12)",
          muted: "oklch(0.62 0.035 8)",
          "muted-foreground": "oklch(0.72 0.035 8)",
          accent: "oklch(0.75 0.16 12)",
          border: "oklch(0.33 0.035 8)",
        },
      },
    },
  },
  {
    // Violet Bloom — the DARK translucent look (pinned by `glass`). Dark carries the identity: a rich
    // deep violet-plum ground with enough chroma that glass panels tint beautifully, a luminous lavender
    // accent, and a clearly-raised plum card. Light is a soft lilac paper.
    value: "violet",
    name: "Violet Bloom",
    preset: {
      label: "Violet Bloom",
      styles: {
        light: {
          background: "oklch(0.99 0.006 300)",
          foreground: "oklch(0.28 0.05 300)",
          card: "oklch(0.965 0.014 300)",
          popover: "oklch(0.965 0.014 300)",
          primary: "oklch(0.53 0.2 296)",
          muted: "oklch(0.52 0.04 300)",
          "muted-foreground": "oklch(0.44 0.04 300)",
          accent: "oklch(0.53 0.2 296)",
          border: "oklch(0.9 0.02 300)",
        },
        dark: {
          background: "oklch(0.19 0.05 300)",
          foreground: "oklch(0.93 0.02 300)",
          card: "oklch(0.25 0.06 298)",
          popover: "oklch(0.25 0.06 298)",
          primary: "oklch(0.76 0.14 296)",
          muted: "oklch(0.62 0.05 298)",
          "muted-foreground": "oklch(0.73 0.05 298)",
          accent: "oklch(0.76 0.14 296)",
          border: "oklch(0.34 0.06 298)",
        },
      },
    },
  },
  {
    // Ocean — the airy LIGHT dashboard (pinned by `modern`). Light = bright page with the faintest cool
    // tint, a vivid cyan-teal accent, crisp cool borders. Dark = deep teal-navy. Distinct from Editor by
    // being genuinely light and more saturated; distinct from Emerald by its cool blue-green hue.
    value: "ocean",
    name: "Ocean",
    preset: {
      label: "Ocean",
      styles: {
        light: {
          // Modern = AIRY & COOL: a soft blue-tinted ground (NOT near-white — the visible cool wash is
          // what separates it at a glance from Professional's warm paper) with white cards floating on
          // it (card is LIGHTER than bg → the airy-dashboard "cards lift off a tinted canvas" look) and
          // a confident sky-blue accent shifted off Glass's plum.
          background: "oklch(0.965 0.018 240)",
          foreground: "oklch(0.27 0.03 250)",
          card: "oklch(0.995 0.004 240)",
          popover: "oklch(0.995 0.004 240)",
          primary: "oklch(0.55 0.17 245)",
          muted: "oklch(0.52 0.03 245)",
          "muted-foreground": "oklch(0.49 0.03 245)",
          accent: "oklch(0.55 0.17 245)",
          border: "oklch(0.89 0.016 240)",
        },
        dark: {
          background: "oklch(0.205 0.02 255)",
          foreground: "oklch(0.93 0.015 250)",
          card: "oklch(0.25 0.025 255)",
          popover: "oklch(0.25 0.025 255)",
          primary: "oklch(0.74 0.14 240)",
          muted: "oklch(0.66 0.03 250)",
          "muted-foreground": "oklch(0.74 0.03 250)",
          accent: "oklch(0.74 0.14 240)",
          border: "oklch(0.32 0.025 252)",
        },
      },
    },
  },
  {
    // Retro terminal — amber phosphor on a near-neutral true black. The `retro` LOOK pins this preset
    // (its identity is the palette). The key move vs. the old green-on-green: neutrals are near-zero
    // chroma so the accent SEPARATES from the ground. "Light" is a warm sepia-paper terminal for parity.
    value: "retro",
    name: "Retro Terminal",
    preset: {
      label: "Retro Terminal",
      styles: {
        light: {
          // Warm sepia-paper terminal for parity (amber ink on aged paper).
          background: "oklch(0.97 0.010 85)",
          foreground: "oklch(0.28 0.04 60)",
          card: "oklch(0.945 0.014 85)",
          popover: "oklch(0.945 0.014 85)",
          primary: "oklch(0.52 0.13 55)",
          muted: "oklch(0.52 0.05 65)",
          "muted-foreground": "oklch(0.46 0.05 65)",
          accent: "oklch(0.52 0.13 55)",
          border: "oklch(0.88 0.02 80)",
        },
        dark: {
          // The whole point of the fix: NEUTRALS DROP TO NEAR-ZERO CHROMA (a true near-black), so the
          // amber phosphor accent reads as a signal instead of green-on-green mud. Amber (not green)
          // also separates cleanly from the green "Ok" status dots.
          background: "oklch(0.145 0.008 75)",
          foreground: "oklch(0.85 0.13 80)",
          card: "oklch(0.18 0.012 75)",
          popover: "oklch(0.18 0.012 75)",
          primary: "oklch(0.80 0.165 72)",
          muted: "oklch(0.60 0.09 82)",
          "muted-foreground": "oklch(0.70 0.09 82)",
          accent: "oklch(0.80 0.165 72)",
          border: "oklch(0.28 0.02 78)",
        },
      },
    },
  },
];
