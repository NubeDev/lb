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
    // The flagship LIGHT look (pinned by the `professional` look). Light = genuine PAPER: a warm near-
    // white leaf, near-black ink, a calm indigo-blue accent, soft cool-grey borders and a real text
    // hierarchy. Dark stays a clean cool slate. The raised card sits a touch off the page so panels read.
    value: "slate",
    name: "Slate",
    preset: {
      label: "Slate",
      styles: {
        light: {
          background: "oklch(0.99 0.004 250)",
          foreground: "oklch(0.24 0.02 264)",
          card: "oklch(0.965 0.006 250)",
          popover: "oklch(0.965 0.006 250)",
          primary: "oklch(0.5 0.16 262)",
          muted: "oklch(0.52 0.02 258)",
          "muted-foreground": "oklch(0.44 0.02 258)",
          accent: "oklch(0.5 0.16 262)",
          border: "oklch(0.9 0.008 258)",
        },
        dark: {
          background: "oklch(0.22 0.02 264)",
          foreground: "oklch(0.95 0.01 264)",
          card: "oklch(0.27 0.02 264)",
          popover: "oklch(0.27 0.02 264)",
          primary: "oklch(0.72 0.14 264)",
          muted: "oklch(0.64 0.02 264)",
          "muted-foreground": "oklch(0.72 0.02 264)",
          accent: "oklch(0.72 0.14 264)",
          border: "oklch(0.34 0.02 264)",
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
          background: "oklch(0.985 0.003 230)",
          foreground: "oklch(0.26 0.02 240)",
          card: "oklch(0.955 0.005 225)",
          popover: "oklch(0.955 0.005 225)",
          primary: "oklch(0.5 0.11 220)",
          muted: "oklch(0.5 0.02 235)",
          "muted-foreground": "oklch(0.42 0.02 235)",
          accent: "oklch(0.5 0.11 220)",
          border: "oklch(0.9 0.006 230)",
        },
        dark: {
          background: "oklch(0.19 0.018 248)",
          foreground: "oklch(0.9 0.01 230)",
          card: "oklch(0.235 0.02 248)",
          popover: "oklch(0.235 0.02 248)",
          primary: "oklch(0.78 0.12 205)",
          muted: "oklch(0.6 0.02 240)",
          "muted-foreground": "oklch(0.68 0.02 235)",
          accent: "oklch(0.78 0.12 205)",
          border: "oklch(0.31 0.018 245)",
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
          background: "oklch(0.99 0.008 210)",
          foreground: "oklch(0.26 0.04 220)",
          card: "oklch(0.965 0.016 205)",
          popover: "oklch(0.965 0.016 205)",
          primary: "oklch(0.52 0.11 220)",
          muted: "oklch(0.5 0.03 215)",
          "muted-foreground": "oklch(0.42 0.03 215)",
          accent: "oklch(0.52 0.11 220)",
          border: "oklch(0.9 0.02 208)",
        },
        dark: {
          background: "oklch(0.19 0.03 220)",
          foreground: "oklch(0.93 0.02 205)",
          card: "oklch(0.24 0.035 220)",
          popover: "oklch(0.24 0.035 220)",
          primary: "oklch(0.8 0.13 200)",
          muted: "oklch(0.62 0.035 210)",
          "muted-foreground": "oklch(0.72 0.035 205)",
          accent: "oklch(0.8 0.13 200)",
          border: "oklch(0.32 0.035 218)",
        },
      },
    },
  },
  {
    // Retro terminal — phosphor green on near-black. The `retro` LOOK pins this preset (its identity is
    // the palette). Both modes are green-forward; "light" is a warm-paper amber terminal for parity.
    value: "retro",
    name: "Retro Terminal",
    preset: {
      label: "Retro Terminal",
      styles: {
        light: {
          background: "#f4f1e8",
          foreground: "#2a2410",
          card: "#ece7d6",
          popover: "#ece7d6",
          primary: "#7a5c00",
          muted: "#6b5f3a",
          "muted-foreground": "#514828",
          accent: "#7a5c00",
          border: "#d8cfb4",
        },
        dark: {
          background: "#0a0f0a",
          foreground: "#3bd67a",
          card: "#0e160e",
          popover: "#0e160e",
          primary: "#38f58a",
          muted: "#5f9e72",
          "muted-foreground": "#7fc496",
          accent: "#38f58a",
          border: "#1a2c1e",
        },
      },
    },
  },
];
