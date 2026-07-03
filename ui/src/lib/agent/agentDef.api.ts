// The agent-definition catalog API client (agent-catalog scope) — one call per verb (FILE-LAYOUT),
// each reached over the same MCP bridge as any verb (rule 7). Mirrors the host `agent.def.*` surface:
//   - `agent.def.list`   → `{ definitions }`  (member; `mcp:agent.def.list:call`)
//   - `agent.def.get`    → `{ definition }`   (member; `mcp:agent.def.get:call`)
//   - `agent.def.create` → `{ ok }`           (admin;  `mcp:agent.def.create:call`)
//   - `agent.def.update` → `{ ok }`           (admin;  `mcp:agent.def.update:call`)
//   - `agent.def.delete` → `{ ok }`           (admin;  `mcp:agent.def.delete:call`)
// NAMES ONLY — `model_endpoint.api_key_env` is an env-var NAME, never a key value.

import { invoke } from "@/lib/ipc/invoke";

/** A model endpoint a definition binds — names only. `provider`/`model` required; the rest optional. */
export interface DefinitionEndpoint {
  provider: string;
  model: string;
  /** The env-var NAME holding the key — never the key value. */
  api_key_env?: string;
  /** A secret PATH into `lb-secrets` holding the key (a name, never the value). Resolved at
   *  model-call time secret→env. The value is written ONLY through `secret.set` (sealed). */
  api_key_secret?: string;
  base_url?: string;
}

/** The assembled run context a `agent.def.test` proved — names only (tool + skill names carry no
 *  secret within the workspace). */
export interface TestContext {
  tool_count: number;
  tools: string[];
  skill_count: number;
  skills: string[];
}

/** The result of the context-proving diagnostic (`agent.def.test`). */
export interface TestResult {
  id: string;
  answer: string;
  runtime: string;
  model: string;
  context: TestContext;
  /** Whether the node's model is a real provider vs. the unconfigured placeholder (honest UI copy). */
  provider_configured: boolean;
  ok: boolean;
}

/** One catalog entry: a named `(runtime, model_endpoint)` preset. `builtin` is set by the host. */
export interface AgentDefinition {
  id: string;
  label: string;
  description?: string;
  runtime: string;
  model_endpoint: DefinitionEndpoint;
  /** True for a seeded read-only built-in; false for a workspace custom entry. */
  builtin: boolean;
}

/** A partial edit to a custom definition — every field optional (absent = unchanged). */
export interface DefinitionPatch {
  label?: string;
  description?: string;
  runtime?: string;
  model_endpoint?: DefinitionEndpoint;
}

/** List the catalog (node-runnable built-ins ∪ workspace custom). Member-level. */
export function listAgentDefs(): Promise<AgentDefinition[]> {
  return invoke<{ definitions: AgentDefinition[] }>("mcp_call", {
    tool: "agent.def.list",
    args: {},
  }).then((r) => r.definitions);
}

/** Read one definition by id. Member-level. */
export function getAgentDef(id: string): Promise<AgentDefinition> {
  return invoke<{ definition: AgentDefinition }>("mcp_call", {
    tool: "agent.def.get",
    args: { id },
  }).then((r) => r.definition);
}

/** Create a custom definition. Admin-gated (opaque deny otherwise). */
export function createAgentDef(def: AgentDefinition): Promise<void> {
  return invoke<{ ok: boolean }>("mcp_call", {
    tool: "agent.def.create",
    args: def,
  }).then(() => undefined);
}

/** Update a custom definition by merging `patch`. Admin-gated. */
export function updateAgentDef(id: string, patch: DefinitionPatch): Promise<void> {
  return invoke<{ ok: boolean }>("mcp_call", {
    tool: "agent.def.update",
    args: { id, patch },
  }).then(() => undefined);
}

/** Delete a custom definition. Admin-gated. */
export function deleteAgentDef(id: string): Promise<void> {
  return invoke<{ ok: boolean }>("mcp_call", {
    tool: "agent.def.delete",
    args: { id },
  }).then(() => undefined);
}

/** Test one definition end to end (the context-proving diagnostic). Admin-gated
 *  (`mcp:agent.def.test:call`) — it spends a model turn. `id` omitted → the active `agent.config`
 *  pick. Returns the model's answer + the assembled context (tool/skill NAMES). */
export function testAgentDef(id?: string): Promise<TestResult> {
  return invoke<TestResult>("mcp_call", {
    tool: "agent.def.test",
    args: id ? { id } : {},
  });
}

/** Set (or rotate) a definition's sealed MODEL KEY. The value flows ONLY through the shipped sealed
 *  `secret.set` (owner-stamped, workspace-scoped); the caller then stores just the resulting `path` on
 *  the definition (names-only). The value is never read back. Returns the path.
 *
 *  Sealed **`workspace`** visibility on purpose: an agent RUN resolves this key under a derived
 *  `agent:` principal (not the admin who set it), so a `Private` key (owner-only) would be unreadable
 *  at run time. `workspace` keeps it behind the workspace wall + the `secret:<path>:get` cap while
 *  letting the run's actor resolve it — overwrite/rotate stays owner-only regardless. */
export function setModelKey(path: string, value: string): Promise<string> {
  return invoke<{ ok?: boolean }>("mcp_call", {
    tool: "secret.set",
    args: { path, value, visibility: "workspace" },
  }).then(() => path);
}
