// The session store — the single source of truth for "who am I / which workspace", held outside
// React so the client layer reads the token without prop-drilling (the `ui/src/lib/session/
// session.store.ts` pattern, extended to one-session-per-workspace). A tiny observable:
// get/set/subscribe; the shell adapts it to React via `useSyncExternalStore`. Persistence goes
// through the injected `SessionStorage` (keychain on device, memory in tests).

import type { SessionStorage } from "./session.storage";
import type { Session, StoredSessions } from "./session.types";

export interface SessionStore {
  /** The active session, or null when logged out. */
  current(): Session | null;
  /** The bearer token of the active session, or `""` — what the request layer attaches. */
  token(): string;
  /** Every workspace holding a stored session (the switcher's local half). */
  workspaces(): string[];
  /** Add/replace the session for its workspace and make it active. */
  activate(session: Session): void;
  /** Switch to an already-stored workspace session. Throws if none is stored for `ws`. */
  switchTo(ws: string): void;
  /** Drop one workspace's session (active falls to any remaining one), or everything. */
  logout(ws?: string): void;
  /** Rehydrate from durable storage (call once on boot). */
  restore(): Promise<void>;
  subscribe(listener: () => void): () => void;
}

export function createSessionStore(storage: SessionStorage): SessionStore {
  let held: StoredSessions | null = null;
  const listeners = new Set<() => void>();

  function commit(next: StoredSessions | null): void {
    held = next;
    void storage.save(next);
    for (const l of listeners) l();
  }

  return {
    current: () => (held ? (held.sessions[held.active] ?? null) : null),
    token() {
      return this.current()?.token ?? "";
    },
    workspaces: () => (held ? Object.keys(held.sessions) : []),
    activate(session) {
      const sessions = { ...(held?.sessions ?? {}), [session.workspace]: session };
      commit({ active: session.workspace, sessions });
    },
    switchTo(ws) {
      if (!held?.sessions[ws]) throw new Error(`no stored session for workspace "${ws}"`);
      commit({ ...held, active: ws });
    },
    logout(ws) {
      if (!held) return;
      if (ws === undefined) return commit(null);
      const sessions = { ...held.sessions };
      delete sessions[ws];
      const remaining = Object.keys(sessions);
      if (remaining.length === 0) return commit(null);
      commit({ active: held.active === ws ? remaining[0] : held.active, sessions });
    },
    async restore() {
      held = await storage.load();
      for (const l of listeners) l();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
