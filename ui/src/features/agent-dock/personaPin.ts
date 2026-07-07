// The dock persona PIN (persona-session #5) — the sticky per-tab override of the page-context match.
// One responsibility: own the pin in `sessionStorage`, keyed by workspace. Per-tab is the WHOLE POINT
// (two members / two tabs are fully independent — scope's headline), so this MUST be `sessionStorage`
// (per-tab) NOT `localStorage` (which is shared across tabs). The key is workspace-namespaced so a
// shared browser with two workspaces keeps their pins distinct. The host has NO server-side tab state
// (scope non-goal: "no server-side session/tab identity") — the pin rides client-side and is sent as
// the per-invoke `persona` arg; the durable job record is the audit.
//
// FILE-LAYOUT: pure storage helpers only — no React, no IPC. The hook that decides the resolved focus
// (pin > context match > none) lives in `usePersonaFocus.ts`.

/** The `sessionStorage` key for the pinned persona in `ws`. Namespaced like the dock chrome's
 *  `lb.agent-dock.*` localStorage keys, but in sessionStorage (per-tab, not per-browser). */
export function pinKey(ws: string): string {
  return `lb.agent-dock.persona-pin.${ws}`;
}

/** Read the pinned persona id for `ws` in THIS tab's sessionStorage (`null` when none / unavailable).
 *  SSR / non-browser guards mirror `useDockChrome.ts`'s `localStorage` guards. */
export function readPersonaPin(ws: string, storage: SessionStorage | undefined = globalThis.sessionStorage): string | null {
  if (typeof storage === "undefined" || storage == null) return null;
  const raw = storage.getItem(pinKey(ws));
  return raw && raw.length > 0 ? raw : null;
}

/** Pin `personaId` for `ws` in this tab. An empty/whitespace id is treated as "clear" (defensive —
 *  callers should use {@link clearPersonaPin} to clear). No-op when sessionStorage is unavailable. */
export function writePersonaPin(ws: string, personaId: string, storage: SessionStorage | undefined = globalThis.sessionStorage): void {
  if (typeof storage === "undefined" || storage == null) return;
  const trimmed = personaId.trim();
  if (!trimmed) {
    storage.removeItem(pinKey(ws));
    return;
  }
  storage.setItem(pinKey(ws), trimmed);
}

/** Clear the pinned persona for `ws` in this tab (returns the focus to the page-context match, or to
 *  the server's prefs fold when no match). No-op when sessionStorage is unavailable. */
export function clearPersonaPin(ws: string, storage: SessionStorage | undefined = globalThis.sessionStorage): void {
  if (typeof storage === "undefined" || storage == null) return;
  storage.removeItem(pinKey(ws));
}

/** The minimal `sessionStorage` surface this module touches (only `getItem`/`setItem`/`removeItem`),
 *  so a test can pass a fake storage WITHOUT re-implementing the whole `Storage` interface. */
export interface SessionStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}
