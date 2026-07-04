// Upgrade a stored/imported CustomTheme (validated as required-complete, but possibly missing the new
// derived tones) into a FULL palette per mode — filling `panel2`/`overlay`/`accent2` by derivation from
// the required seven. This is the load-bearing migration step: a v1 custom theme (seven tokens, no
// widened tones) survives and gains the new tones instead of failing closed to DEFAULT_THEME.
//
// One responsibility: partial-tones CustomTheme → full-palette CustomTheme.

import { completeTones } from "./derive-tones";
import type { CustomTheme, RequiredPalette } from "./theme-tokens";

/** Fill both modes' derived tones from their required seven, preserving any tones already present. */
export function completeCustomTheme(theme: CustomTheme): CustomTheme {
  return {
    light: fillMode(theme.light),
    dark: fillMode(theme.dark),
  };
}

function fillMode(palette: RequiredPalette): CustomTheme["light"] {
  // `palette` is required-complete; any derived tones already present on it are kept, the rest derived.
  const present = palette as Partial<CustomTheme["light"]> & RequiredPalette;
  return {
    ...palette,
    ...completeTones(palette, { panel2: present.panel2, overlay: present.overlay, accent2: present.accent2 }),
  };
}
