// Durable persistence for the session so a page refresh doesn't drop the login (collaboration scope,
// slice 1). The session store (`session.store.ts`) holds the token in memory for the IPC layer; this
// file mirrors it into `localStorage` so the token survives a reload and new tabs, matching the
// existing theme/recent-extensions persistence pattern. The stored value is the signed bearer the
// gateway issued — it re-checks every verb server-side (§7), so a stale token simply fails auth and
// the UI falls back to login. One responsibility per file (FILE-LAYOUT): load/save/clear only.

import type { Session } from "./session.types";

const SESSION_STORAGE_KEY = "lb.session";

interface SessionStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

function isSession(value: unknown): value is Session {
  if (typeof value !== "object" || value === null) return false;
  const s = value as Record<string, unknown>;
  return typeof s.token === "string" && typeof s.principal === "string" && typeof s.workspace === "string";
}

/** The persisted session, or `null` when none is stored or the stored value is malformed. */
export function loadSession(storage: SessionStorage | undefined = globalThis.localStorage): Session | null {
  if (!storage) return null;

  try {
    const raw = storage.getItem(SESSION_STORAGE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    return isSession(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

/** Persist the session (login) or clear it (logout, when `next` is `null`). */
export function saveSession(next: Session | null, storage: SessionStorage | undefined = globalThis.localStorage): void {
  if (!storage) return;

  try {
    if (next) storage.setItem(SESSION_STORAGE_KEY, JSON.stringify(next));
    else storage.removeItem(SESSION_STORAGE_KEY);
  } catch {
    // Local storage can be unavailable in private modes or locked-down webviews.
  }
}
