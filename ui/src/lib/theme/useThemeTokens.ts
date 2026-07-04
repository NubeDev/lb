// The React seam for `ctx.theme` — resolves the widened `ThemeTokens` off the live DOM and RE-RESOLVES
// whenever the theme changes (subscribing to the single `lb:themechange` emitter). `features/ext-host`
// (via ExtWidget) reads this and hands it to every mounted widget through `ctx.theme` + `update(ctx)`,
// so a canvas widget recolors in place on a theme change with no re-mount. Resolving from the DOM (not
// just the preference) honors custom/imported/inline colors.
//
// One responsibility: current theme → live-resolved ThemeTokens (re-resolved on change).

import { useMemo, useSyncExternalStore } from "react";

import { onThemeChange } from "./theme-events";
import { resolveThemeTokens, type ThemeTokens } from "./resolve-theme-tokens";
import { DEFAULT_THEME } from "./theme-options";
import { useThemeOptional } from "./useTheme";

// A monotonically-bumped counter is the external store's snapshot; it changes on every theme emit so a
// subscribed component re-renders and recomputes the tokens off the freshly-written DOM.
let version = 0;
onThemeChange(() => {
  version += 1;
});
const getSnapshot = () => version;

/** A resolved `ThemeTokens` that updates on every theme change. The object identity changes only when the
 *  version or the preference does, so a downstream `useMemo`/`update(ctx)` fires exactly on a real change. */
export function useThemeTokens(): ThemeTokens {
  // Degrade to DEFAULT_THEME outside a ThemeProvider (a standalone-mounted ext widget) — the tokens still
  // resolve off the live DOM, so the widget gets the applied colors even without the provider in its tree.
  const ctx = useThemeOptional();
  const theme = ctx?.theme ?? DEFAULT_THEME;
  const v = useSyncExternalStore(onThemeChange, getSnapshot, getSnapshot);
  // eslint-disable-next-line react-hooks/exhaustive-deps -- `v` bumps on every theme emit; `theme` covers a same-DOM pref change
  return useMemo(() => resolveThemeTokens(theme), [v, theme]);
}
