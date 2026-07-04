// The curated font list — DATA, not branches (FILE-LAYOUT). Each family is a row: a stable id, a label,
// the CSS `font-family` stack written to `--font-sans`/`--font-mono`, and the `@fontsource` package id
// used to LAZY-load the woff2 on selection (system stacks have no package — zero-cost default). The
// `role` marks which token slot a family fills. Adding a family is adding a row; no code changes.
//
// Self-hosted (no CDN — the shell must boot offline and inside Tauri), latin subset, loaded on first
// selection and preloaded only when the stored theme names one. See `font-loader.ts` for the loader.
//
// One responsibility: the font family catalog.

export type FontRole = "sans" | "mono" | "serif";

export interface FontFamily {
  /** Stable id stored in the preference + `data`-less; also the picker value AND the `font-loader` key
   *  for the lazy `@fontsource` import (a system stack has no loader entry). */
  id: string;
  label: string;
  role: FontRole;
  /** The CSS stack written to the token — the family first, then a robust system fallback. */
  stack: string;
  /** True when the family ships a self-hosted `@fontsource` bundle to lazy-load; false = system stack. */
  selfHosted: boolean;
}

const SYSTEM_SANS = "ui-sans-serif, system-ui, -apple-system, \"Segoe UI\", sans-serif";
const SYSTEM_MONO = "ui-monospace, SFMono-Regular, \"SF Mono\", Menlo, Consolas, monospace";

/** The sans families. `system` is the shipped default (no download). */
export const SANS_FONTS: readonly FontFamily[] = [
  { id: "system", label: "System", role: "sans", stack: SYSTEM_SANS, selfHosted: false },
  { id: "inter", label: "Inter", role: "sans", stack: `"Inter", ${SYSTEM_SANS}`, selfHosted: true },
  { id: "geist", label: "Geist", role: "sans", stack: `"Geist", ${SYSTEM_SANS}`, selfHosted: true },
  {
    id: "ibm-plex-sans",
    label: "IBM Plex Sans",
    role: "sans",
    stack: `"IBM Plex Sans", ${SYSTEM_SANS}`,
    selfHosted: true,
  },
  {
    id: "source-serif-4",
    label: "Source Serif 4",
    role: "serif",
    stack: `"Source Serif 4", ui-serif, Georgia, serif`,
    selfHosted: true,
  },
];

/** The mono families. `system` is the shipped default (no download). */
export const MONO_FONTS: readonly FontFamily[] = [
  { id: "system-mono", label: "System mono", role: "mono", stack: SYSTEM_MONO, selfHosted: false },
  {
    id: "jetbrains-mono",
    label: "JetBrains Mono",
    role: "mono",
    stack: `"JetBrains Mono", ${SYSTEM_MONO}`,
    selfHosted: true,
  },
  {
    id: "ibm-plex-mono",
    label: "IBM Plex Mono",
    role: "mono",
    stack: `"IBM Plex Mono", ${SYSTEM_MONO}`,
    selfHosted: true,
  },
];

export const ALL_FONTS: readonly FontFamily[] = [...SANS_FONTS, ...MONO_FONTS];

export const DEFAULT_FONT_SANS = "system";
export const DEFAULT_FONT_MONO = "system-mono";

/** Look up a family by id, or undefined. */
export function fontById(id: string): FontFamily | undefined {
  return ALL_FONTS.find((f) => f.id === id);
}
