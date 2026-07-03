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
  setModelKey,
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
  /** Seal a model key (token) for the ACTIVE pick — even a built-in — without cloning it. Seals the
   *  value through `secret.set` (Private) and writes only the resulting PATH onto `agent.config`. */
  setActiveKey: (value: string) => Promise<void>;
  create: (def: AgentDefinition) => Promise<void>;
  update: (id: string, patch: DefinitionPatch) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

/** Does the active `config` resolve to `def`? The first-class `active_definition` id is authoritative
 *  when present (the pick writes it — active-agent-wiring scope); otherwise fall back to matching the
 *  copied runtime + endpoint (provider + model) for a config written before the field existed. */
function matchesActive(def: AgentDefinition, config: AgentConfig): boolean {
  if (config.active_definition) return config.active_definition === def.id;
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
      // `resolve_effective_runtime` already honors `default_runtime`, so no new resolution seam. The
      // `active_definition` id makes "which agent is active" first-class (active-agent-wiring scope):
      // the badge, rules, and the per-workspace model resolver read it instead of re-deriving.
      await setAgentConfig({
        active_definition: def.id,
        default_runtime: def.runtime,
        model_endpoint: {
          provider: def.model_endpoint.provider,
          model: def.model_endpoint.model,
          api_key_env: def.model_endpoint.api_key_env,
          // A custom definition may already carry a sealed key path — preserve it on pick.
          api_key_secret: def.model_endpoint.api_key_secret,
          base_url: def.model_endpoint.base_url,
        },
      });
      reload();
    },
    [reload],
  );

  const setActiveKey = useCallback(
    async (value: string) => {
      // Seal the token for the ACTIVE pick without cloning a built-in: the active `agent.config` is
      // workspace-scoped and can own a sealed secret path (scope open-question #5). The value flows
      // ONLY through `secret.set` (Private, owner-stamped); we store just the resulting PATH on
      // `agent.config`. The path is stable per active endpoint so a rotate overwrites (owner-only).
      const rt = config.default_runtime ?? "default";
      const model = config.model_endpoint?.model ?? "model";
      const path = `agent/config-${rt}-${model}-key`.replace(/[^a-z0-9/_-]+/gi, "-");
      await setModelKey(path, value);
      // Merge onto the existing endpoint — a partial patch keeps provider/model/env/base intact.
      await setAgentConfig({
        model_endpoint: { ...(config.model_endpoint ?? {}), api_key_secret: path },
      });
      reload();
    },
    [config, reload],
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

  return {
    definitions,
    runtimes,
    config,
    activeId,
    loading,
    reload,
    pick,
    setActiveKey,
    create,
    update,
    remove,
  };
}
