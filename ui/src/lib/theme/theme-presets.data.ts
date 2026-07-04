// The built-in preset LIBRARY — DATA, not code (FILE-LAYOUT: presets are data files, never branches).
// A curated, contrast-vetted subset of the shadcn/tweakcn packs (the scope's recommendation: ship a
// vetted subset for the library, keep Import for the long tail). Each entry is a `ThemePreset` in the
// shadcn vocabulary; the adapter maps it onto base tokens at apply-time. Adding a preset = adding a row
// here. The three BUILTIN accents (amber/teal/blue) are NOT here — they apply via `data-theme-accent`
// and live in `globals.css`; this file is the *library* beyond them.

import type { PresetEntry } from "./theme-preset";

export const THEME_PRESETS: readonly PresetEntry[] = [
  {
    value: "slate",
    name: "Slate",
    preset: {
      label: "Slate",
      styles: {
        light: {
          background: "oklch(1 0 0)",
          foreground: "oklch(0.21 0.03 264)",
          card: "oklch(0.98 0.01 248)",
          popover: "oklch(0.98 0.01 248)",
          primary: "oklch(0.55 0.18 264)",
          muted: "oklch(0.55 0.02 264)",
          "muted-foreground": "oklch(0.45 0.02 264)",
          accent: "oklch(0.55 0.18 264)",
          border: "oklch(0.9 0.01 264)",
        },
        dark: {
          background: "oklch(0.2 0.02 264)",
          foreground: "oklch(0.95 0.01 264)",
          card: "oklch(0.24 0.02 264)",
          popover: "oklch(0.24 0.02 264)",
          primary: "oklch(0.7 0.16 264)",
          muted: "oklch(0.65 0.02 264)",
          "muted-foreground": "oklch(0.7 0.02 264)",
          accent: "oklch(0.7 0.16 264)",
          border: "oklch(0.32 0.02 264)",
        },
      },
    },
  },
  {
    value: "emerald",
    name: "Emerald",
    preset: {
      label: "Emerald",
      styles: {
        light: {
          background: "#ffffff",
          foreground: "#0f2419",
          card: "#f4faf6",
          popover: "#f4faf6",
          primary: "#059669",
          muted: "#5b7a6b",
          "muted-foreground": "#456156",
          accent: "#059669",
          border: "#d9e8df",
        },
        dark: {
          background: "#0d1a13",
          foreground: "#e6f2ea",
          card: "#12241a",
          popover: "#12241a",
          primary: "#34d399",
          muted: "#7fa08e",
          "muted-foreground": "#8fb3a0",
          accent: "#34d399",
          border: "#1f3a2a",
        },
      },
    },
  },
  {
    value: "rose",
    name: "Rose",
    preset: {
      label: "Rose",
      styles: {
        light: {
          background: "#fffafb",
          foreground: "#3d0e1a",
          card: "#fdf0f3",
          popover: "#fdf0f3",
          primary: "#e11d48",
          muted: "#8a5563",
          "muted-foreground": "#6f4451",
          accent: "#e11d48",
          border: "#f4d9e0",
        },
        dark: {
          background: "#1a0d11",
          foreground: "#f8e6ec",
          card: "#24131a",
          popover: "#24131a",
          primary: "#fb7185",
          muted: "#a67f8a",
          "muted-foreground": "#b8909b",
          accent: "#fb7185",
          border: "#3a1f28",
        },
      },
    },
  },
  {
    value: "violet",
    name: "Violet Bloom",
    preset: {
      label: "Violet Bloom",
      styles: {
        light: {
          background: "#fdfcff",
          foreground: "#241436",
          card: "#f6f1fc",
          popover: "#f6f1fc",
          primary: "#7c3aed",
          muted: "#6d5b86",
          "muted-foreground": "#574470",
          accent: "#7c3aed",
          border: "#e6dcf5",
        },
        dark: {
          background: "#140d1f",
          foreground: "#ece4f7",
          card: "#1d1330",
          popover: "#1d1330",
          primary: "#a78bfa",
          muted: "#8f80a8",
          "muted-foreground": "#a091b8",
          accent: "#a78bfa",
          border: "#2e2142",
        },
      },
    },
  },
  {
    value: "ocean",
    name: "Ocean",
    preset: {
      label: "Ocean",
      styles: {
        light: {
          background: "#f8fdff",
          foreground: "#0a2a33",
          card: "#edf8fb",
          popover: "#edf8fb",
          primary: "#0891b2",
          muted: "#4f7681",
          "muted-foreground": "#3d5d66",
          accent: "#0891b2",
          border: "#d3e9ef",
        },
        dark: {
          background: "#08191f",
          foreground: "#e0f2f7",
          card: "#0e242c",
          popover: "#0e242c",
          primary: "#22d3ee",
          muted: "#7ba0aa",
          "muted-foreground": "#8fb3bd",
          accent: "#22d3ee",
          border: "#1a3a44",
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
