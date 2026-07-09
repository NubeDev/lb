// The rules editor "auto-format" preference (rules-editor-ux scope). One responsibility: own the
// on/off flag in `localStorage` so a user's choice to auto-format Rhai on save/blur survives reloads
// and is shared across tabs (a client-only editor convenience — NOT a server/member pref, so it
// stays out of `lb_prefs`; the server has no stake in a browser formatting habit). Namespaced under
// the same `lb.rules.*` convention as the app's other client-side keys.
//
// FILE-LAYOUT: pure storage helpers only — no React. The hook that exposes it as reactive state
// lives in `useAutoFormat.ts`.

/** The `localStorage` key for the rules editor auto-format flag (browser-wide, not per-workspace —
 *  a formatting habit is the user's, the same in every workspace). */
export const AUTO_FORMAT_KEY = "lb.rules.auto-format";

/** The minimal `localStorage` surface this module touches, so a test can pass a fake WITHOUT
 *  re-implementing the whole `Storage` interface. */
export interface LocalStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** Read the auto-format flag (default `false` — the safe, non-surprising default: a user opts IN).
 *  SSR / non-browser guards mirror the other client-side stores. */
export function readAutoFormat(storage: LocalStorage | undefined = globalThis.localStorage): boolean {
  if (typeof storage === "undefined" || storage == null) return false;
  return storage.getItem(AUTO_FORMAT_KEY) === "1";
}

/** Persist the auto-format flag. No-op when localStorage is unavailable. */
export function writeAutoFormat(
  on: boolean,
  storage: LocalStorage | undefined = globalThis.localStorage,
): void {
  if (typeof storage === "undefined" || storage == null) return;
  storage.setItem(AUTO_FORMAT_KEY, on ? "1" : "0");
}
