// The session store — the single source of truth for "who am I / which workspace", held outside
// React so the IPC layer (`http.ts`, `channel.stream.ts`) can read the token without prop-drilling
// it through every call (collaboration scope, slice 1). One store per domain (FILE-LAYOUT: never a
// generic `store.ts`). A tiny observable: `get`/`set`/`subscribe` — no state library needed for one
// value, and `useSession` adapts it to React via `useSyncExternalStore`.

import { loadSession, saveSession } from "./session.storage";
import type { Session } from "./session.types";

// Rehydrate from durable storage so a page refresh keeps the login (the token lives outside React and
// outside this module's lifetime). The gateway re-checks the token on every verb, so a stale/expired
// one just fails auth and the UI falls back to login.
let current: Session | null = loadSession();
const listeners = new Set<() => void>();

/** The current session, or `null` when logged out. Read by the IPC layer on every request. */
export function getSession(): Session | null {
  return current;
}

/** The bearer token for the current session, or `""` when logged out — what `http.ts` attaches. */
export function sessionToken(): string {
  return current?.token ?? "";
}

/** Replace the session (login → a `Session`, logout → `null`) and notify subscribers. */
export function setSession(next: Session | null): void {
  current = next;
  saveSession(next);
  for (const l of listeners) l();
}

/** Subscribe to session changes (for `useSyncExternalStore`). Returns an unsubscribe fn. */
export function subscribeSession(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
