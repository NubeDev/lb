// The Agent-catalog data hook (agent-catalog scope) — loads the catalog (`agent.def.list`), the node's
// runnable runtimes (`agent.runtimes`, for the editor + registry-drift flag), and the active selection
// (`agent.config.get`, resolved from `default_runtime` + `model_endpoint`). Exposes the mutations
// (create/update/delete + pick) that write the real verbs and reload. One responsibility: the data
// plumbing for the catalog manager; the presentation lives in the components.

import { useCallback, useEffect, useState } from "react";

import { agentRuntimes, type AgentRuntimes } from "@/lib/agent/runtimes.api";
import { getAgentConfig, setAgentConfig, type AgentConfig } from "@/lib/agent/config.api";
import {
  listAgentDefs,
  createAgentDef,
  updateAgentDef,
  deleteAgentDef,
  type AgentDefinition,
  type DefinitionPatch,
} from "@/lib/agent/agentDef.api";

export interface AgentCatalog {
  definitions: AgentDefinition[];
  runtimes: AgentRuntimes | null;
  config: AgentConfig;
  /** The id of the definition that matches the active selection, if any (highlighted in the list). */
  activeId: string | null;
  loading: boolean;
  reload: () => void;
  /** Pick a definition as the workspace default — writes `agent.config` from its runtime + endpoint. */
  pick: (def: AgentDefinition) => Promise<void>;
  create: (def: AgentDefinition) => Promise<void>;
  update: (id: string, patch: DefinitionPatch) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

/** Does the active `config` resolve to `def`? A definition is active when its runtime AND endpoint
 *  (provider + model) match the stored selection — the copy the pick wrote into `agent.config`. */
function matchesActive(def: AgentDefinition, config: AgentConfig): boolean {
  if (config.default_runtime !== def.runtime) return false;
  const ep = config.model_endpoint;
  return (
    !!ep &&
    ep.provider === def.model_endpoint.provider &&
    ep.model === def.model_endpoint.model
  );
}

export function useAgentCatalog(): AgentCatalog {
  const [definitions, setDefinitions] = useState<AgentDefinition[]>([]);
  const [runtimes, setRuntimes] = useState<AgentRuntimes | null>(null);
  const [config, setConfig] = useState<AgentConfig>({});
  const [loading, setLoading] = useState(true);

  const reload = useCallback(() => {
    setLoading(true);
    void Promise.allSettled([listAgentDefs(), agentRuntimes(), getAgentConfig()]).then(
      ([defs, rts, cfg]) => {
        setDefinitions(defs.status === "fulfilled" ? defs.value : []);
        setRuntimes(rts.status === "fulfilled" ? rts.value : null);
        setConfig(cfg.status === "fulfilled" ? (cfg.value ?? {}) : {});
        setLoading(false);
      },
    );
  }, []);
  useEffect(reload, [reload]);

  const activeId =
    definitions.find((d) => matchesActive(d, config))?.id ?? null;

  const pick = useCallback(
    async (def: AgentDefinition) => {
      // Picking writes the shipped `agent.config` from the definition's fields — the invoke path's
      // `resolve_effective_runtime` already honors `default_runtime`, so no new resolution seam.
      await setAgentConfig({
        default_runtime: def.runtime,
        model_endpoint: {
          provider: def.model_endpoint.provider,
          model: def.model_endpoint.model,
          api_key_env: def.model_endpoint.api_key_env,
          base_url: def.model_endpoint.base_url,
        },
      });
      reload();
    },
    [reload],
  );

  const create = useCallback(
    async (def: AgentDefinition) => {
      await createAgentDef(def);
      reload();
    },
    [reload],
  );
  const update = useCallback(
    async (id: string, patch: DefinitionPatch) => {
      await updateAgentDef(id, patch);
      reload();
    },
    [reload],
  );
  const remove = useCallback(
    async (id: string) => {
      await deleteAgentDef(id);
      reload();
    },
    [reload],
  );

  return { definitions, runtimes, config, activeId, loading, reload, pick, create, update, remove };
}
