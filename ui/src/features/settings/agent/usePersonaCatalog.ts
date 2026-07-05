// The persona-catalog data hook (agent-personas scope #1) — loads the persona catalog
// (`agent.persona.list`) and the active selection (`agent.config.get` → `active_persona`), and exposes
// the mutations (create/update/delete + pick) that write the real verbs and reload. One responsibility:
// the data plumbing for the persona manager; the presentation lives in the components. Mirrors
// `useAgentCatalog` (the definition catalog's hook) — personas are orthogonal to definitions (persona
// picks FOCUS, definition picks the runtime/model), so this is a parallel pointer on `agent.config`.

import { useCallback, useEffect, useState } from "react";

import { getAgentConfig, setAgentConfig, type AgentConfig } from "@/lib/agent/config.api";
import {
  listPersonas,
  createPersona,
  updatePersona,
  deletePersona,
  type Persona,
  type PersonaPatch,
} from "@/lib/agent/agentPersona.api";

export interface PersonaCatalog {
  personas: Persona[];
  config: AgentConfig;
  /** The id of the persona matching `agent.config.active_persona`, if it still resolves. */
  activeId: string | null;
  loading: boolean;
  reload: () => void;
  /** Pick a persona as the workspace default — writes `agent.config.active_persona`. */
  pick: (persona: Persona) => Promise<void>;
  create: (persona: Persona) => Promise<void>;
  update: (id: string, patch: PersonaPatch) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

export function usePersonaCatalog(): PersonaCatalog {
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [config, setConfig] = useState<AgentConfig>({});
  const [loading, setLoading] = useState(true);

  const reload = useCallback(() => {
    setLoading(true);
    void Promise.allSettled([listPersonas(), getAgentConfig()]).then(([ps, cfg]) => {
      setPersonas(ps.status === "fulfilled" ? ps.value : []);
      setConfig(cfg.status === "fulfilled" ? (cfg.value ?? {}) : {});
      setLoading(false);
    });
  }, []);
  useEffect(reload, [reload]);

  // The active persona is the first-class `active_persona` pointer (the `active_definition` move
  // exactly) — a stored fact, not re-derived. Null when unset or when the stored id no longer lists.
  const activeId =
    (config.active_persona && personas.some((p) => p.id === config.active_persona)
      ? config.active_persona
      : null) ?? null;

  const pick = useCallback(
    async (persona: Persona) => {
      // Picking writes only the `active_persona` pointer — the invoke path's `resolve` reads it at run
      // assembly; no new resolution seam. Additive + optional, exactly like `active_definition`.
      await setAgentConfig({ active_persona: persona.id });
      reload();
    },
    [reload],
  );

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

  return { personas, config, activeId, loading, reload, pick, create, update, remove };
}
