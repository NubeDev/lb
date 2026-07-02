import { create } from "zustand";

// Presence — other collaborators connected to the same engine. The engine is
// an opaque relay: it stores each session's last-published `state` and fans it
// out to everyone else. We define the state SHAPE here (the client owns the
// schema; the engine never parses it), so it can grow (cursor, viewport, …)
// without an engine change.

export interface PresenceState {
  // Display name. Client-chosen for now (localStorage); could come from auth
  // later. The engine doesn't care.
  userName?: string;
  // Component uids this collaborator currently has selected. Drives the
  // "someone else is on this node" ring.
  selectedComponents?: number[];
  // Parent uid of the folder they're viewing — lets us scope the selection
  // ring to "only when we're looking at the same folder".
  parentUid?: number;
}

export interface Collaborator {
  sessionId: string;
  state: PresenceState;
  // Stable per-session color, assigned on first sight. Index into PALETTE.
  colorIdx: number;
  // performance.now() of the last presence message from this session. Drives
  // TTL expiry: the engine only evicts dead sessions on a 60s grace timer and
  // can lag (stale "others" pile up across reconnects), so the client ages out
  // any collaborator not heard from within the TTL. Live peers heartbeat, so
  // they stay; dead ones age out.
  lastSeen: number;
}

// Distinct, dark-canvas-legible hues. Assigned round-robin as sessions appear.
export const PRESENCE_PALETTE = [
  "hsl(var(--amber))", // orange
  "hsl(var(--r1))", // purple
  "hsl(var(--cool))", // cyan
  "hsl(var(--r1))", // pink
  "hsl(var(--green))", // lime
  "hsl(var(--amber))", // yellow
  "hsl(var(--crit))", // red
  "hsl(var(--r2))", // teal
];

interface PresenceStore {
  // Keyed by sessionId. Never includes our own session (engine doesn't echo).
  collaborators: Map<string, Collaborator>;
  version: number;
  upsert(sessionId: string, state: PresenceState): void;
  remove(sessionId: string): void;
  replaceAll(entries: Array<{ sessionId: string; state: PresenceState }>): void;
  // Drop collaborators not heard from within ttlMs. Called on an interval.
  sweep(ttlMs: number): void;
  reset(): void;
}

let nextColor = 0;
const nowMs = () => performance.now();

export const usePresence = create<PresenceStore>((set, get) => ({
  collaborators: new Map(),
  version: 0,
  upsert: (sessionId, state) =>
    set((s) => {
      const next = new Map(s.collaborators);
      const existing = next.get(sessionId);
      next.set(sessionId, {
        sessionId,
        state,
        colorIdx: existing ? existing.colorIdx : nextColor++ % PRESENCE_PALETTE.length,
        lastSeen: nowMs(),
      });
      return { collaborators: next, version: s.version + 1 };
    }),
  sweep: (ttlMs) =>
    set((s) => {
      const cutoff = nowMs() - ttlMs;
      let changed = false;
      const next = new Map(s.collaborators);
      for (const [id, c] of next) {
        if (c.lastSeen < cutoff) {
          next.delete(id);
          changed = true;
        }
      }
      return changed ? { collaborators: next, version: s.version + 1 } : s;
    }),
  remove: (sessionId) =>
    set((s) => {
      if (!s.collaborators.has(sessionId)) return s;
      const next = new Map(s.collaborators);
      next.delete(sessionId);
      return { collaborators: next, version: s.version + 1 };
    }),
  replaceAll: (entries) =>
    set((s) => {
      const next = new Map<string, Collaborator>();
      for (const e of entries) {
        const existing = get().collaborators.get(e.sessionId);
        next.set(e.sessionId, {
          sessionId: e.sessionId,
          state: e.state as PresenceState,
          colorIdx: existing ? existing.colorIdx : nextColor++ % PRESENCE_PALETTE.length,
          lastSeen: nowMs(),
        });
      }
      return { collaborators: next, version: s.version + 1 };
    }),
  reset: () => set({ collaborators: new Map(), version: 0 }),
}));

// Build a uid → collaborator[] index for "who has this component selected",
// scoped to a given parent folder so we don't ring a node for someone who has
// the same uid selected in a different view (uids are unique so this is mostly
// belt-and-braces, but the parent scope also future-proofs cursor sharing).
export function selectionIndex(
  collaborators: Map<string, Collaborator>,
  parentUid: number,
): Map<number, Collaborator[]> {
  const idx = new Map<number, Collaborator[]>();
  for (const c of collaborators.values()) {
    if (c.state.parentUid !== undefined && c.state.parentUid !== parentUid) continue;
    for (const uid of c.state.selectedComponents ?? []) {
      let arr = idx.get(uid);
      if (!arr) {
        arr = [];
        idx.set(uid, arr);
      }
      arr.push(c);
    }
  }
  return idx;
}
