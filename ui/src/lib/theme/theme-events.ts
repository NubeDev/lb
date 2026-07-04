// The theme change emitter — a tiny shell-internal pub/sub (NOT a DOM CustomEvent, NOT localStorage).
// `theme-dom` fires it after every application; `features/ext-host` is the SINGLE subscriber that
// resolves the computed tokens once and fans them out to every mounted extension (rule 10: no extension
// is named; each gets the same signal). One emitter, one fan-out — the theme-inheritance-scope contract.
//
// One responsibility: the `lb:themechange` pub/sub.

type ThemeChangeListener = () => void;

const listeners = new Set<ThemeChangeListener>();

/** Subscribe to theme changes. Returns an unsubscribe fn. */
export function onThemeChange(listener: ThemeChangeListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** Fire a theme change — called by `theme-dom` after it writes the DOM. Listeners resolve the new
 *  computed tokens themselves (the emitter carries no payload; the DOM is the source of truth). */
export function emitThemeChange(): void {
  for (const listener of [...listeners]) listener();
}
