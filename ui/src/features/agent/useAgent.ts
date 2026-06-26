// The agent hook — data + state for invoking the central agent (FILE-LAYOUT: one hook per file,
// data separated from markup). Invokes through the capability-checked node verb; a denied invoke
// (no `mcp:agent.invoke:call`, or an ungranted substrate skill) surfaces as an error — the same
// gate the Rust `agent_test` proves, surfaced to the user, never a silent empty.

import { useCallback, useState } from "react";

import { invokeAgent } from "@/lib/agent/agent.api";
import type { AgentResult } from "@/lib/agent/agent.types";

export interface AgentState {
  result: AgentResult | null;
  running: boolean;
  /** Set when the node refused the invoke (denied) — shown to the user. */
  error: string | null;
  /** Invoke the agent with a goal (and optional granted skill). */
  run: (goal: string, skill?: string) => Promise<void>;
}

/** Drive the central agent in `(ws)` as `author` holding `caps` (the demo session identity +
 *  grant until real login lands — see agent.api). `jobId` is the durable session id. */
export function useAgent(
  ws: string,
  jobId: string,
  author: string,
  caps: string[],
): AgentState {
  const [result, setResult] = useState<AgentResult | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(
    async (goal: string, skill?: string) => {
      setRunning(true);
      try {
        setResult(await invokeAgent(ws, jobId, goal, { skill, author, caps }));
        setError(null);
      } catch (e) {
        setResult(null);
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setRunning(false);
      }
    },
    [ws, jobId, author, caps],
  );

  return { result, running, error, run };
}
