// The dock SESSION-id persistence (agent-dock scope) — remembers which dock conversation was last
// open so a page refresh (or a new tab) reopens where the user left off, instead of minting a fresh
// empty session every mount. localStorage, NOT sessionStorage: a dock session is a per-USER
// conversation thread (the channel id is `dock-{user-slug}-{ulid}`), so it is safe and desirable to
// share it across tabs and browser restarts — the user clicks "New session" to branch. (Contrast
// the persona pin in `personaPin.ts`, which is sessionStorage because per-tab persona independence
// was that scope's headline.)
//
// The key is namespaced by BOTH workspace AND user-slug, so a shared browser never loads one user's
// dock conversation under another's login — the storage key mirrors the channel id's own scoping.
//
// FILE-LAYOUT: pure storage helpers only — no React, no IPC. The hook that decides whether the
// restored id is still valid (and mints fresh otherwise) lives in `useDockSessions.ts`. Mirrors
// `personaPin.ts`'s shape (the sibling per-tab store) one-to-one; the difference is the storage
// backend (localStorage vs sessionStorage) and the added user-slug dimension.

import { userSlug } from "./dockId";

/** The `localStorage` key for the last-open dock session in `ws` for `principal`. Namespaced like
 *  the dock chrome's `lb.agent-dock.*` keys, plus the user-slug so a shared browser keeps users
 *  distinct (the channel id is already user-scoped; the storage key must match). */
export function sessionKey(ws: string, principal: string): string {
  return `lb.agent-dock.session.${ws}.${userSlug(principal)}`;
}

/** Read the last-open dock session id for `ws`/`principal` (`null` when none / unavailable).
 *  SSR / non-browser guards mirror `useDockChrome.ts`'s `localStorage` guards. */
export function readDockSession(
  ws: string,
  principal: string,
  storage: LocalStorage | undefined = globalThis.localStorage,
): string | null {
  if (typeof storage === "undefined" || storage == null) return null;
  const raw = storage.getItem(sessionKey(ws, principal));
  return raw && raw.length > 0 ? raw : null;
}

/** Remember `cid` as the last-open dock session for `ws`/`principal`. An empty/whitespace id is
 *  treated as "clear" (defensive — callers should use {@link clearDockSession} to clear). No-op
 *  when localStorage is unavailable. */
export function writeDockSession(
  ws: string,
  principal: string,
  cid: string,
  storage: LocalStorage | undefined = globalThis.localStorage,
): void {
  if (typeof storage === "undefined" || storage == null) return;
  const trimmed = cid.trim();
  if (!trimmed) {
    storage.removeItem(sessionKey(ws, principal));
    return;
  }
  storage.setItem(sessionKey(ws, principal), trimmed);
}

/** Forget the last-open dock session for `ws`/`principal`. No-op when localStorage is unavailable. */
export function clearDockSession(
  ws: string,
  principal: string,
  storage: LocalStorage | undefined = globalThis.localStorage,
): void {
  if (typeof storage === "undefined" || storage == null) return;
  storage.removeItem(sessionKey(ws, principal));
}

/** The minimal `localStorage` surface this module touches (only `getItem`/`setItem`/`removeItem`),
 *  so a test can pass a fake storage WITHOUT re-implementing the whole `Storage` interface. */
export interface LocalStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}
