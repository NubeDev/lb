// The agent-runtimes API client — one call per export (FILE-LAYOUT). Reads the node's configured
// agent runtimes for the composer runtime picker (external-agent run-lifecycle #5), reached over the
// same MCP bridge as any verb (rule 7): `agent.runtimes` returns `{ default, runtimes }` — the ids
// this node has registered plus the default id. Mirrors `lb_host::list_runtimes`. Gated by
// `mcp:agent.runtimes:call`; a caller without it is denied opaquely by the host.

import { invoke } from "@/lib/ipc/invoke";

/** The `agent.runtimes` response — the node's configured runtime ids (sorted) + the default id. */
export interface AgentRuntimes {
  /** The default runtime id (`"default"` — the in-house loop). Preselected in the picker. */
  default: string;
  /** All configured runtime ids, sorted; `default` is always among them. */
  runtimes: string[];
}

/** List the node's configured agent runtimes for the caller's workspace (the picker's one read). */
export function agentRuntimes(): Promise<AgentRuntimes> {
  return invoke<AgentRuntimes>("mcp_call", { tool: "agent.runtimes", args: {} });
}
