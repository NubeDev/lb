// The editor's color palette (editor-parity scope, step 2) — the Grafana semantic color names offered
// by the color swatch pickers (thresholds, value-mapping colors, fixed color), each resolved to its
// theme CSS via `fieldconfig/color.ts` so a swatch paints the real rendered color. One responsibility:
// the name→swatch list.

import { resolveColor } from "../../fieldconfig/color";
import type { Swatch } from "@/components/ui/color-swatch";

/** The semantic color names (the render path in `color.ts` knows how to resolve). */
export const PALETTE_NAMES = ["green", "yellow", "orange", "red", "blue", "purple"] as const;

/** The swatch list for a `ColorSwatchPicker` — each name painted as its resolved theme color. */
export const COLOR_SWATCHES: Swatch[] = PALETTE_NAMES.map((value) => ({ value, css: resolveColor(value) }));
