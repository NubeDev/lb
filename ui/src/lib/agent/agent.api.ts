// The agent API client — one call per export, mirroring the Rust agent verb
// (`lb_host::invoke` / the `agent.invoke` MCP tool) and the node command name one-to-one. The UI
// never calls `invoke` directly; it goes through this named verb (FILE-LAYOUT frontend rules).
//
// `author` is the caller's principal. The REAL node derives it from the session token and ignores
// this arg (the gateway demo-principal is a STATUS.md open question); the in-memory fake uses it to
// resolve the invoke capability gate, so the UI's allow/deny paths are exercised in tests exactly
// as the node would. `caps` lets a test set the caller's grant for the same reason.

import type { AgentResult } from "./agent.types";
import { invoke } from "@/lib/ipc/invoke";

/** Invoke the central agent in `ws` with a goal (optionally over a granted skill / shared doc).
 *  Mirrors `lb_host::invoke` reached as the `agent.invoke` MCP tool. `opts.persona` (persona-session
 *  #5) is the explicit per-invoke focus override — the dock sends the same id via the channel-agent
 *  body; absent ⇒ the server folds member→ws-default prefs (may land on none). */
export function invokeAgent(
  ws: string,
  jobId: string,
  goal: string,
  opts?: { skill?: string; doc?: string; persona?: string; author?: string; caps?: string[] },
): Promise<AgentResult> {
  return invoke<AgentResult>("agent_invoke", {
    ws,
    jobId,
    goal,
    skill: opts?.skill,
    doc: opts?.doc,
    persona: opts?.persona,
    author: opts?.author,
    caps: opts?.caps,
  });
}
