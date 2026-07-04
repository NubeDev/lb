// Resolve a `ThemePreference` to the concrete base palette that should be applied for the ACTIVE mode
// — the single decision of "what colors does this preference mean right now". Precedence (highest
// first): `custom` (hand-tweaked brand colors) → `imported` (pasted tweakcn) → a library `preset` →
// a built-in accent preset (null palette: applied via `data-theme-accent`, not inline). Keeping this
// pure and separate from `theme-dom` means the DOM layer just writes what it's handed, and this is
// unit-testable without a document.
//
// One responsibility: ThemePreference + mode → (BasePalette | null).

import { adaptPreset } from "./preset-adapter";
import { isBuiltinPreset, type ThemePreference } from "./theme-options";
import { THEME_PRESETS } from "./theme-presets.data";
import type { BasePalette } from "./theme-tokens";

/** The base palette to write inline for `pref` in the active mode, or null when the preference is a
 *  built-in accent (which needs no inline write — `globals.css` + `data-theme-accent` own it). */
export function resolvePalette(pref: ThemePreference): BasePalette | null {
  const mode = pref.mode;
  if (pref.custom) return pref.custom[mode];
  if (pref.imported) return pref.imported[mode];

  // A built-in accent preset applies via the attribute, not inline.
  if (isBuiltinPreset(pref.preset)) return null;

  // A library preset — adapt it to base tokens for this mode.
  const entry = THEME_PRESETS.find((p) => p.value === pref.preset);
  if (entry) {
    const adapted = adaptPreset(entry.preset);
    if (adapted) return adapted[mode];
  }
  // Unknown preset id → fall back to the built-in accent path (no inline write).
  return null;
}

/** The `data-theme-accent` value for `pref`: the built-in accent name, or undefined when a custom/
 *  imported/library palette is driving the colors inline (the attribute must then be cleared so it
 *  doesn't fight the inline `--accent`). */
export function resolveAccentAttr(pref: ThemePreference): string | undefined {
  if (pref.custom || pref.imported) return undefined;
  if (isBuiltinPreset(pref.preset)) return pref.preset;
  // Library preset applied inline → no attribute.
  return undefined;
}
