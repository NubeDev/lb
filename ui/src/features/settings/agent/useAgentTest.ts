// The Test-a-definition hook (agent-catalog test-and-secrets scope) — drives the `agent.def.test`
// context-proving diagnostic for one definition (or the active pick) and holds the transient result.
// One responsibility: the async plumbing for the Test button; the presentation lives in the component.
//
// The test SPENDS a model turn (admin-gated). It returns the model's single-turn answer plus the
// REAL assembled context (tool + skill names) so the admin sees the agent was given its Lazybones
// context even against the deterministic mock provider.

import { useCallback, useState } from "react";

import { testAgentDef, type TestResult } from "@/lib/agent/agentDef.api";

export interface AgentTest {
  /** The result of the last test for THIS definition, or null before/while running. */
  result: TestResult | null;
  running: boolean;
  error: string | null;
  /** Run the test for `id` (omit for the active `agent.config` pick). */
  run: (id?: string) => Promise<void>;
  /** Clear the shown result (e.g. when collapsing the panel). */
  clear: () => void;
}

export function useAgentTest(): AgentTest {
  const [result, setResult] = useState<TestResult | null>(null);
  const [running, setRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (id?: string) => {
    setRunning(true);
    setError(null);
    setResult(null);
    try {
      setResult(await testAgentDef(id));
    } catch (e) {
      setError(e instanceof Error ? e.message : "test failed");
    } finally {
      setRunning(false);
    }
  }, []);

  const clear = useCallback(() => {
    setResult(null);
    setError(null);
  }, []);

  return { result, running, error, run, clear };
}
