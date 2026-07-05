// The agent-policy API client (agent-run Part 2 machinery, first Settings surface in agent-personas #1)
// — the per-tool Allow / Ask / Deny supervision rules, reached over the same MCP bridge as any verb
// (rule 7):
//   - `agent.policy.get` → `{ rules }`         (member read; `mcp:agent.policy.get:call`)
//   - `agent.policy.set` → `{ ok, rules: N }`  (admin;       `mcp:agent.policy.set:call`)
// This edits SUPERVISION (how a run is watched), never the wall — an Ask/Deny rule tightens what the
// agent may do unattended; it does not grant or revoke a capability. Tool globs are OPAQUE (rule 10).

import { invoke } from "@/lib/ipc/invoke";

/** How a matched tool call is supervised. `deny` blocks it; `ask` suspends the run for approval;
 *  `allow` lets it through (the default when no rule matches). */
export type Effect = "allow" | "deny" | "ask";

/** An optional argument match narrowing a rule to calls whose `path` equals `equals`. */
export interface ArgMatch {
  path: string;
  equals: string;
}

/** One supervision rule: a tool id / trailing-`*` glob, an optional arg match, and the effect. */
export interface Rule {
  tool: string;
  arg?: ArgMatch;
  effect: Effect;
}

/** Read the workspace's agent-policy rules. Member-level (read). */
export function getAgentPolicy(): Promise<Rule[]> {
  return invoke<{ rules: Rule[] }>("mcp_call", {
    tool: "agent.policy.get",
    args: {},
  }).then((r) => r.rules);
}

/** Replace the workspace's agent-policy rules. Admin-gated (opaque deny otherwise). Returns the count
 *  the host stored. */
export function setAgentPolicy(rules: Rule[]): Promise<number> {
  return invoke<{ ok: boolean; rules: number }>("mcp_call", {
    tool: "agent.policy.set",
    args: { rules },
  }).then((r) => r.rules);
}
