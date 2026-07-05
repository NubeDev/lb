// The dock SESSION list + current-session selection (agent-dock scope) — the picker's data. Lists the
// user's OWN `dock.{user-slug}.*` channels (channel.list filtered to the user's prefix), tracks which
// session is current, and mints a fresh one for "New session". One responsibility: session identity +
// the picker list; the messages of the current session live in `useDockSession`.
//
// "New session" mints a fresh ulid and makes it current; the old channel stays listable/reopenable (no
// delete in v1). A freshly-minted session is REGISTERED eagerly (`channel.create`, idempotent) the moment
// it becomes current — so an opened-but-not-yet-posted session survives a reload and stays reselectable in
// the picker, rather than vanishing until its first post (create-on-first-post left abandoned sessions
// unlistable). Registration is best-effort: the local list already shows the current session immediately,
// and the durable `refresh` reconciles it; a denied `create` (no pub cap) just leaves the session
// local-only, exactly as before, and the first post surfaces the real capability error.

import { useCallback, useEffect, useMemo, useState } from "react";

import { createChannel, listChannels } from "@/lib/channel/channel.api";
import { dockPrefixFor, isOwnDockChannel, mintDockId } from "./dockId";

export interface DockSessionsState {
  /** The user's dock session ids, newest-first (the picker list). Includes the current unposted one. */
  sessions: string[];
  /** The currently-open dock session channel id. */
  current: string;
  /** Select an existing session (reopen a past one). */
  select: (cid: string) => void;
  /** Mint a fresh session and make it current (the old one stays listable). */
  newSession: () => void;
  error: string | null;
}

/** Drive the dock's sessions for `ws` / `principal`. Loads the user's own `dock.` channels; seeds a
 *  fresh current session on first mount so the dock always has somewhere to post. */
export function useDockSessions(ws: string, principal: string): DockSessionsState {
  // The current session id — minted once on first mount (lazy init) so opening the dock always lands
  // on a usable (possibly empty, uncreated) session. Reset only via select/newSession.
  const [current, setCurrent] = useState<string>(() => mintDockId(principal));
  const [listed, setListed] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const rows = await listChannels(ws);
      // Only the user's OWN dock sessions (a member can read any workspace channel, but the picker is
      // scoped to their prefix — scope: "the dock picker includes only the user's own prefix").
      const ids = rows.map((r) => r.id).filter((id) => isOwnDockChannel(id, principal));
      setListed(ids);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [ws, principal]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Register the current session eagerly so an opened-but-unposted session is durable + reselectable
  // (not create-on-first-post). Idempotent host-side (re-create upserts the same row), so re-running on
  // every `current` change is safe. Best-effort: a denial (no pub cap) is swallowed here — the session
  // stays local-only and the first post raises the real capability error, unchanged.
  useEffect(() => {
    let cancelled = false;
    void createChannel(ws, current)
      .then(() => {
        if (!cancelled) setListed((prev) => (prev.includes(current) ? prev : [...prev, current]));
      })
      .catch(() => {
        /* denied / offline — the local list already shows `current`; the first post reports the error */
      });
    return () => {
      cancelled = true;
    };
  }, [ws, current]);

  // The picker list: the durable sessions plus the (possibly not-yet-created) current one, deduped,
  // newest-first. Dock ids are lexicographically time-ordered (the ulid), so a reverse sort is newest.
  const sessions = useMemo(() => {
    const set = new Set<string>(listed);
    set.add(current);
    return [...set].filter((id) => id.startsWith(dockPrefixFor(principal))).sort().reverse();
  }, [listed, current, principal]);

  const select = useCallback((cid: string) => setCurrent(cid), []);
  const newSession = useCallback(() => setCurrent(mintDockId(principal)), [principal]);

  return { sessions, current, select, newSession, error };
}
