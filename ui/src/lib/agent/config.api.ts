// The per-workspace agent-config API client (agent-config scope) — read/write the workspace's chosen
// default runtime + model endpoint, reached over the same MCP bridge as any verb (rule 7):
//   - `agent.config.get` → `{ config }` (member-level; `mcp:agent.config.get:call`)
//   - `agent.config.set` → `{ ok }`     (admin-only; `mcp:agent.config.set:call`)
// Mirrors `lb_host::agent_config_get/set`. Names only — `api_key_env` is an env-var NAME, never a key.

import { invoke } from "@/lib/ipc/invoke";

/** A model endpoint the workspace's agent routes through — names only, all optional (a patch). */
export interface ModelEndpointPatch {
  provider?: string;
  model?: string;
  /** The env-var NAME holding the key — never the key value. */
  api_key_env?: string;
  /** A secret PATH into `lb-secrets` holding the key (a name, never the value). Lets a workspace key
   *  its ACTIVE-pick model without cloning a built-in. Resolved at model-call time secret→env. */
  api_key_secret?: string;
  base_url?: string;
}

/** The stored/patch shape of a workspace's agent config. Mirrors `lb_host::AgentConfig` (nullable). */
export interface AgentConfig {
  /** The chosen default runtime id (must be one the node offers — validated on write). */
  default_runtime?: string;
  model_endpoint?: ModelEndpointPatch;
  /** The active definition id the workspace picked (active-agent-wiring scope) — first-class so
   *  "which agent is active" is a stored fact, not re-derived from `default_runtime` + endpoint.
   *  Written by the pick alongside the copied fields; additive + optional (back-compat). */
  active_definition?: string;
  /** The workspace's ENABLED persona roster (persona-session #5 — replaces the single `active_persona`
   *  toggle). `undefined` (default) ⇒ ALL personas enabled (built-ins + customs, on-by-default out of
   *  the box); an array ⇒ only those ids. Curation of the advertisement layer: a disabled persona is
   *  hidden from `agent.persona.list`'s picker view and from the dock's context match, and an explicit
   *  invoke of one fails with a named disabled error. Same record, same MERGE patch, same admin gate. */
  enabled_personas?: string[];
}

/** Read the workspace's agent config (`null` when unset). Member-level. */
export function getAgentConfig(): Promise<AgentConfig | null> {
  return invoke<{ config: AgentConfig | null }>("mcp_call", {
    tool: "agent.config.get",
    args: {},
  }).then((r) => r.config);
}

/** Merge `patch` into the workspace's agent config. Admin-gated by the host (opaque deny otherwise). */
export function setAgentConfig(patch: AgentConfig): Promise<void> {
  return invoke<{ ok: boolean }>("mcp_call", {
    tool: "agent.config.set",
    args: { patch },
  }).then(() => undefined);
}
