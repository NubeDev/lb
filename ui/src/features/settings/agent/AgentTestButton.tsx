// The per-definition Test button + inline result (agent-catalog test-and-secrets scope). Clicking it
// runs `agent.def.test` — a single-turn invoke with the caller's REAL context assembled (system
// prompt + reachable MCP/ACP tools + granted skills) — and shows the model's reply plus a compact
// "context: N tools, M skills" line, so an admin can confirm the agent has its Lazybones context.
//
// Honest copy: against the deterministic mock/unconfigured provider the ANSWER is canned, so the
// CONTEXT LINE is what proves the agent was given its tools/skills. `provider_configured` drives a
// truthful "responding via the configured provider" vs. "no model provider is wired" note — never
// implying a real LLM answered when the placeholder did.

import { Button } from "@/components/ui/button";
import { useAgentTest } from "./useAgentTest";

interface Props {
  /** The definition id to test. */
  id: string;
}

export function AgentTestButton({ id }: Props) {
  const { result, running, error, run } = useAgentTest();

  return (
    <div className="flex flex-col items-end gap-1">
      <Button
        size="sm"
        variant="outline"
        onClick={() => void run(id)}
        disabled={running}
        aria-label={`test ${id}`}
      >
        {running ? "Testing…" : "Test"}
      </Button>

      {error && (
        <span role="alert" className="text-[11px] text-red-500">
          {error}
        </span>
      )}

      {result && (
        <div
          className="mt-1 w-full max-w-md rounded-md border border-border bg-panel/50 p-2 text-left"
          aria-label={`test result ${id}`}
        >
          <p className="whitespace-pre-wrap text-xs text-fg">{result.answer}</p>
          <p className="mt-1.5 text-[11px] text-muted">
            context: {result.context.tool_count} tools, {result.context.skill_count} skills
            {" · "}
            {result.provider_configured
              ? "responding via the configured provider"
              : "no model provider is wired — the answer is a placeholder"}
          </p>
        </div>
      )}
    </div>
  );
}
