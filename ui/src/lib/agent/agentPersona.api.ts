// The agent-persona catalog API client (agent-personas scope, sub-scope #1) — one call per verb
// (FILE-LAYOUT), each reached over the same MCP bridge as any verb (rule 7). Mirrors the host
// `agent.persona.*` surface:
//   - `agent.persona.list`    → `{ personas }`          (member; `mcp:agent.persona.list:call`)
//   - `agent.persona.get`     → `{ persona }`           (member; `mcp:agent.persona.get:call`)
//   - `agent.persona.resolve` → `{ effective }`         (member; `mcp:agent.persona.resolve:call`)
//   - `agent.persona.create`  → `{ ok }`                (admin;  `mcp:agent.persona.create:call`)
//   - `agent.persona.update`  → `{ ok }`                (admin;  `mcp:agent.persona.update:call`)
//   - `agent.persona.delete`  → `{ ok }`                (admin;  `mcp:agent.persona.delete:call`)
// Tool ids inside `granted_tools`/`grounding_skills`/`extends` are OPAQUE strings (rule 10) — the UI
// never branches on a specific id, it only lists/matches them as data.

import { invoke } from "@/lib/ipc/invoke";

/** A per-persona supervision floor (persona-catalog #4): tools the persona wants Ask'd / Deny'd. The
 *  policy pane shows these as the FLOOR — tightening is free, loosening is the explicit admin write. */
export interface PolicyPreset {
  ask: string[];
  deny: string[];
}

/** A workspace-facing persona bundle. Mirrors the Rust `Persona` struct. `builtin` is set by the host
 *  (a seeded read-only `builtin.<slug>`); custom personas are workspace-scoped with admin CRUD.
 *  `surfaces` (persona-session #5) is the page-surface vocabulary the dock's context match reads —
 *  opaque strings compared for equality against `context.surface` over the enabled roster (rule 10). */
export interface Persona {
  id: string;
  label: string;
  description?: string;
  /** The short persona prompt, prepended to the system prompt / folded into the goal. */
  identity: string;
  /** Tool ids or trailing-`*` globs — OPAQUE data (rule 10). Narrows the ADVERTISED menu, never the wall. */
  granted_tools: string[];
  /** Skill ids pinned at session start (grant-gated, fail-closed). */
  grounding_skills: string[];
  /** Persona ids whose tool/skill lists union in (identity: child wins). */
  extends: string[];
  /** Page-surface strings this persona is "for" (persona-session #5) — opaque data the dock's client-side
   *  context match reads (`surface ∈ persona.surfaces` over the enabled roster). Built-ins declare theirs
   *  in `personas.toml`; an empty list = never context-suggested (system-manager is the fallback map). */
  surfaces: string[];
  policy_preset?: PolicyPreset;
  runtimes?: string[];
  /** True for a seeded read-only built-in; false for a workspace custom entry. */
  builtin: boolean;
}

/** A list row: the persona plus its `enabled` flag computed against the workspace roster
 *  (persona-session #5). `agent.persona.list` computes this server-side so the dock/picker fetch is
 *  one read — `enabled: false` ⇒ hidden from the dock's context match + switcher. */
export type PersonaListItem = Persona & { enabled: boolean };

/** A partial edit to a custom persona — every field optional (absent = unchanged). A PRESENT list
 *  REPLACES the stored one (not a merge of entries). `id`/`builtin` are never patchable. */
export type PersonaPatch = Partial<Omit<Persona, "id" | "builtin">>;

/** The extends-unioned, resolved persona the run assembly actually applies — identity + tools + skills
 *  the caller would get. Mirrors the Rust `EffectivePersona`. */
export interface EffectivePersona {
  id: string;
  identity: string;
  granted_tools: string[];
  grounding_skills: string[];
  policy_preset?: PolicyPreset;
  runtimes?: string[];
}

/** List the persona catalog (seeded built-ins ∪ workspace custom), each row carrying its `enabled`
 *  flag computed against the workspace roster (persona-session #5). Member-level; the dock + Settings
 *  picker both read this single fetch. A denied `agent.persona.list` rejects — callers that want to
 *  tolerate a missing roster (the dock) wrap with `.catch(() => [])`. */
export function listPersonas(): Promise<PersonaListItem[]> {
  return invoke<{ personas: PersonaListItem[] }>("mcp_call", {
    tool: "agent.persona.list",
    args: {},
  }).then((r) => r.personas);
}

/** Read one persona by id. Member-level. */
export function getPersona(id: string): Promise<Persona> {
  return invoke<{ persona: Persona }>("mcp_call", {
    tool: "agent.persona.get",
    args: { id },
  }).then((r) => r.persona);
}

/** Resolve the effective (extends-unioned) persona — identity + pinned tools/skills the run gets.
 *  Absent `id` → the workspace's active persona. `null` when nothing resolves. Member-level. */
export function resolveEffectivePersona(id?: string): Promise<EffectivePersona | null> {
  return invoke<{ effective: EffectivePersona | null }>("mcp_call", {
    tool: "agent.persona.resolve",
    args: id ? { id } : {},
  }).then((r) => r.effective);
}

/** Create a custom persona (whole record). Admin-gated (opaque deny otherwise). */
export function createPersona(persona: Persona): Promise<void> {
  return invoke<{ ok: boolean }>("mcp_call", {
    tool: "agent.persona.create",
    args: persona,
  }).then(() => undefined);
}

/** Update a custom persona by REPLACING each present field of `patch`. Admin-gated. */
export function updatePersona(id: string, patch: PersonaPatch): Promise<void> {
  return invoke<{ ok: boolean }>("mcp_call", {
    tool: "agent.persona.update",
    args: { id, patch },
  }).then(() => undefined);
}

/** Delete a custom persona. Admin-gated. */
export function deletePersona(id: string): Promise<void> {
  return invoke<{ ok: boolean }>("mcp_call", {
    tool: "agent.persona.delete",
    args: { id },
  }).then(() => undefined);
}
