// The persona-catalog data hook (agent-personas scope #1, reworked by persona-session #5) — loads the
// persona roster (`agent.persona.list`, now with per-row `enabled`) + the workspace roster config
// (`agent.config.get` → `enabled_personas`) + the viewer's OWN default persona (`prefs.get` →
// `agent_persona`). Exposes the mutations: roster enable/disable (admin `agent.config.set`), member
// default (`prefs.set`), workspace default (`prefs.set_default`), and custom-persona CRUD. The
// workspace-default id is tracked OPTIMISTICALLY (no ws-default read verb exists) — a display nicety
// only; the host's `resolve_persona` is the source of truth at run time.
//
// One responsibility: data plumbing for the persona manager; presentation lives in the components.
// Persona ids are OPAQUE data (rule 10) — no branch on a specific id, only list membership.

import { useCallback, useEffect, useState } from "react";

import { getAgentConfig, setAgentConfig, type AgentConfig } from "@/lib/agent/config.api";
import {
  listPersonas,
  createPersona,
  updatePersona,
  deletePersona,
  type PersonaListItem,
  type Persona,
  type PersonaPatch,
} from "@/lib/agent/agentPersona.api";
import { getPrefs } from "@/lib/prefs/get";
import { setPrefs, setDefaultPrefs } from "@/lib/prefs/set";

export interface PersonaCatalog {
  /** The roster, each row carrying its server-computed `enabled` flag against the workspace roster. */
  personas: PersonaListItem[];
  /** The workspace agent config — carries `enabled_personas` (the raw roster the Settings editor reads). */
  config: AgentConfig;
  /** The viewer's OWN default persona id (`prefs.get` → `agent_persona`), or `null` when unset. */
  memberDefaultId: string | null;
  /** The workspace-default persona id, tracked OPTIMISTICALLY (no ws-default read verb). `null` until
   *  an admin sets one in THIS session; a reload forgets it (display only — the host fold is the truth). */
  wsDefaultId: string | null;
  loading: boolean;
  reload: () => void;
  /** Toggle a persona in the workspace roster. Disabling the last enabled one is refused (an empty
   *  roster means "all enabled" — disabling all is unsupported by design). Admin-gated. */
  toggleEnabled: (persona: PersonaListItem) => Promise<void>;
  /** Set the viewer's OWN default persona (`prefs.set { agent_persona }`). Member-level. */
  setMemberDefault: (id: string) => Promise<void>;
  /** Clear the viewer's own default (writes `""` — the MERGE-can't-write-null workaround). */
  clearMemberDefault: () => Promise<void>;
  /** Set the workspace-default persona (`prefs.set_default { agent_persona }`). Admin-gated. */
  setWsDefault: (id: string) => Promise<void>;
  /** Clear the workspace default (writes `""`). Admin-gated. */
  clearWsDefault: () => Promise<void>;
  create: (persona: Persona) => Promise<void>;
  update: (id: string, patch: PersonaPatch) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

/** Compute the next `enabled_personas` value for a roster toggle. `undefined` (or `[]`) ⇒ all enabled;
 *  a non-empty array ⇒ only those ids. Disabling the last enabled persona is refused (returns the
 *  current list) — an empty roster means "all enabled", so disabling-all is unsupported by design. */
function nextRoster(
  current: string[] | undefined,
  allIds: string[],
  id: string,
  enable: boolean,
): string[] | undefined {
  const all = new Set(allIds);
  const isAllEnabled = !current || current.length === 0;
  if (enable) {
    if (isAllEnabled) return undefined; // already all-enabled — no-op
    const next = new Set([...current!, id]);
    // If we just tipped back to "every persona enabled", clear to undefined (= all).
    if ([...all].every((x) => next.has(x))) return undefined;
    return [...next].sort();
  }
  // disable
  if (isAllEnabled) {
    // Materialize all-but-this (the roster was implicit "all", now becomes explicit "all but id").
    const next = [...allIds].filter((x) => x !== id).sort();
    return next.length === 0 ? [id] : next; // refuse to disable the only persona
  }
  const next = current!.filter((x) => x !== id);
  if (next.length === 0) return [id]; // can't disable all — keep this one enabled
  return [...next].sort();
}

export function usePersonaCatalog(): PersonaCatalog {
  const [personas, setPersonas] = useState<PersonaListItem[]>([]);
  const [config, setConfig] = useState<AgentConfig>({});
  const [memberDefaultId, setMemberDefaultId] = useState<string | null>(null);
  const [wsDefaultId, setWsDefaultId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  const reload = useCallback(() => {
    setLoading(true);
    // `getPrefs` returns the viewer's OWN stored record — the right shape for "did this member set a
    // default" (unset axis = inherit; the agent_persona axis is null unless they explicitly set it).
    void Promise.allSettled([listPersonas(), getAgentConfig(), getPrefs()]).then(
      ([ps, cfg, prefs]) => {
        setPersonas(ps.status === "fulfilled" ? ps.value : []);
        setConfig(cfg.status === "fulfilled" ? (cfg.value ?? {}) : {});
        if (prefs.status === "fulfilled") {
          const axis = prefs.value?.agent_persona;
          // An empty string is "cleared" (the MERGE workaround) — treat as unset for display.
          setMemberDefaultId(axis && axis.length > 0 ? axis : null);
        }
        setLoading(false);
      },
    );
  }, []);
  useEffect(reload, [reload]);

  // A ws-default read verb does NOT exist; wsDefaultId is optimistic only — forget it on reload so we
  // never claim a stale value. (The host's prefs fold is the source of truth at run time.)
  useEffect(() => {
    setWsDefaultId(null);
  }, [reload]);

  const toggleEnabled = useCallback(
    async (persona: PersonaListItem) => {
      const allIds = personas.map((p) => p.id);
      const next = nextRoster(config.enabled_personas, allIds, persona.id, !persona.enabled);
      // Write only `enabled_personas` — MERGE patch, additive axis, exactly like the old `active_persona`
      // pointer move (now retired). The host computes `enabled` per-row on the next `agent.persona.list`.
      await setAgentConfig(next ? { enabled_personas: next } : { enabled_personas: [] });
      reload();
    },
    [personas, config.enabled_personas, reload],
  );

  const setMemberDefault = useCallback(
    async (id: string) => {
      await setPrefs({ agent_persona: id });
      setMemberDefaultId(id);
    },
    [],
  );
  const clearMemberDefault = useCallback(async () => {
    await setPrefs({ agent_persona: "" }); // "" clears the axis (MERGE-can't-write-null workaround)
    setMemberDefaultId(null);
  }, []);

  const setWsDefault = useCallback(
    async (id: string) => {
      await setDefaultPrefs({ agent_persona: id });
      setWsDefaultId(id); // optimistic — no ws-default read verb to confirm
    },
    [],
  );
  const clearWsDefault = useCallback(async () => {
    await setDefaultPrefs({ agent_persona: "" });
    setWsDefaultId(null);
  }, []);

  const create = useCallback(
    async (persona: Persona) => {
      await createPersona(persona);
      reload();
    },
    [reload],
  );
  const update = useCallback(
    async (id: string, patch: PersonaPatch) => {
      await updatePersona(id, patch);
      reload();
    },
    [reload],
  );
  const remove = useCallback(
    async (id: string) => {
      await deletePersona(id);
      reload();
    },
    [reload],
  );

  return {
    personas,
    config,
    memberDefaultId,
    wsDefaultId,
    loading,
    reload,
    toggleEnabled,
    setMemberDefault,
    clearMemberDefault,
    setWsDefault,
    clearWsDefault,
    create,
    update,
    remove,
  };
}
