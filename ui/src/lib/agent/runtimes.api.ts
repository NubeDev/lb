// The agent-runtimes API client — one call per export (FILE-LAYOUT). Reads the node's configured
// agent runtimes for the composer runtime picker (external-agent run-lifecycle #5), reached over the
// same MCP bridge as any verb (rule 7): `agent.runtimes` returns
// `{ default, runtimes, workspace_default }` — the ids this node has registered plus the default id,
// plus the WORKSPACE's active pick (or null). Mirrors `lb_host::list_runtimes`. Gated by
// `mcp:agent.runtimes:call`; a caller without it is denied opaquely by the host.

import { invoke } from "@/lib/ipc/invoke";

/** The workspace's active runtime pick — its id + a human LABEL (resolved from the agent catalog), so
 *  the composer can render "Active — <label>" without a second fetch. Null when no pick is set. */
export interface WorkspaceDefaultRuntime {
  runtime: string;
  label: string;
}

/** The `agent.runtimes` response — the node's configured runtime ids (sorted) + the default id + the
 *  workspace's active pick. */
export interface AgentRuntimes {
  /** The registry default runtime id (`"default"` — the in-house loop). The effective active pick
   *  when the workspace has chosen none. */
  default: string;
  /** All configured runtime ids, sorted; `default` is always among them. */
  runtimes: string[];
  /** The workspace's active pick (id + label), or null when the workspace has chosen no runtime. */
  workspace_default?: WorkspaceDefaultRuntime | null;
}

/** List the node's configured agent runtimes for the caller's workspace (the picker's one read). */
export function agentRuntimes(): Promise<AgentRuntimes> {
  return invoke<AgentRuntimes>("mcp_call", { tool: "agent.runtimes", args: {} });
}
