// The `@`-mention entity listers (channels-command-palette scope). Each `x-lb-entity` arg maps to
// an EXISTING list verb (no new read verb — scope non-goal): `datasource` → `datasource.list`,
// `channel` → `channel_list`, `member` → `members`. The list is fetched once per entity kind and
// cached stale-while-revalidate; typing filters the cache locally (parsePalette does the ranking),
// so a keystroke never hits the network. An empty list returns a REASON, never a spinner (the
// "no dead-ends" criterion). One hook per file (FILE-LAYOUT) — data only.

import { useCallback, useEffect, useRef, useState } from "react";

import { listDatasources } from "@/lib/datasources";
import { listChannels } from "@/lib/channel/channel.api";
import { listMembers } from "@/lib/members/members.api";
import type { EntityKind } from "@/lib/channel/palette.types";
import type { Candidate } from "./parsePalette";

/** The loaded mention set for one entity kind. `reason` is set when `items` is empty (why), so the
 *  menu shows "No datasources you can query" instead of a blank box. */
export interface MentionSet {
  items: Candidate[];
  loading: boolean;
  reason: string | null;
}

const EMPTY: MentionSet = { items: [], loading: false, reason: null };

/** Fetch the candidate list for `entity`, or an empty set with a reason when nothing is reachable.
 *  `table` is intentionally unsupported here (it needs a source first; the SQL widget discovers
 *  tables itself) — it returns a reason rather than a dead spinner. */
async function fetchEntity(entity: EntityKind): Promise<MentionSet> {
  switch (entity) {
    case "datasource": {
      const ds = await listDatasources();
      return ds.length
        ? { items: ds.map((d) => ({ value: d.name, label: d.name, hint: d.kind })), loading: false, reason: null }
        : { items: [], loading: false, reason: "No datasources you can query" };
    }
    case "channel": {
      // ws is derived from the session token server-side (the hard wall); the arg is ignored on the
      // gateway path but the lister's signature requires it.
      const chans = await listChannels("");
      return chans.length
        ? { items: chans.map((c) => ({ value: c.id, label: c.id })), loading: false, reason: null }
        : { items: [], loading: false, reason: "No channels you can reference" };
    }
    case "member": {
      const members = await listMembers("default");
      return members.length
        ? { items: members.map((m) => ({ value: m, label: m })), loading: false, reason: null }
        : { items: [], loading: false, reason: "No members you can mention" };
    }
    default:
      return { items: [], loading: false, reason: "Pick a source first" };
  }
}

/** Load the mention candidates for `entity` (null = no active picker). Stale-while-revalidate: the
 *  previously-loaded set is kept visible while a refresh runs, so the menu never flickers empty. */
export function useMentions(entity: EntityKind | null): MentionSet {
  const [set, setSet] = useState<MentionSet>(EMPTY);
  // Cache per entity kind across re-renders so re-opening the picker is instant (SWR).
  const cache = useRef<Map<EntityKind, MentionSet>>(new Map());

  const load = useCallback(async (kind: EntityKind) => {
    const cached = cache.current.get(kind);
    setSet(cached ? { ...cached, loading: true } : { ...EMPTY, loading: true });
    try {
      const next = await fetchEntity(kind);
      cache.current.set(kind, next);
      setSet(next);
    } catch (e) {
      setSet({ items: [], loading: false, reason: e instanceof Error ? e.message : String(e) });
    }
  }, []);

  useEffect(() => {
    if (entity === null) {
      setSet(EMPTY);
      return;
    }
    void load(entity);
  }, [entity, load]);

  return set;
}
