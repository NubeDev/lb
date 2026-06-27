// The session store — the single source of truth for "who am I / which workspace", held outside
// React so the IPC layer (`http.ts`, `channel.stream.ts`) can read the token without prop-drilling
// it through every call (collaboration scope, slice 1). One store per domain (FILE-LAYOUT: never a
// generic `store.ts`). A tiny observable: `get`/`set`/`subscribe` — no state library needed for one
// value, and `useSession` adapts it to React via `useSyncExternalStore`.

import type { Session } from "./session.types";

let current: Session | null = null;
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
  for (const l of listeners) l();
}

/** Subscribe to session changes (for `useSyncExternalStore`). Returns an unsubscribe fn. */
export function subscribeSession(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
