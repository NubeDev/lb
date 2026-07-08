// The "Generate with agent" card on the Studio Build tab (external-agent-authoring scope S4).
// Pre-fills an `agent.invoke` with `persona:"builtin.extension-builder"` + the chosen runtime,
// then deep-links to the run feed. No new verb — it is sugar over the shipped invoke, exactly as
// the scope decides ("no Studio wizard step; one entry point, not a fourth"). When the external-
// agent feature is off (no external runtimes registered), the card stays hidden (the runtime list
// is empty); the manual Create→Build→Publish wizard below it is untouched.

import { useEffect, useState } from "react";
import { Bot, Loader2, Sparkles } from "lucide-react";

import { Button } from "@/components/ui/button";
import { invokeAgent } from "@/lib/agent/agent.api";
import type { AgentResult } from "@/lib/agent/agent.types";
import { agentRuntimes } from "@/lib/agent/runtimes.api";

interface Props {
  ws: string;
  /** Navigate to the run feed after invoke (deep-link to `/runs/{jobId}`). */
  onOpenRun?: (jobId: string) => void;
}

const PERSONA = "builtin.extension-builder";

/** The external runtime ids (everything except `"default"` — the in-house loop). Empty when the
 *  feature is off (the node was built without `external-agent`); the card hides in that case. */
function useExternalRuntimes(): string[] {
  const [runtimes, setRuntimes] = useState<string[]>([]);
  useEffect(() => {
    let alive = true;
    agentRuntimes()
      .then((r) => {
        if (!alive) return;
        setRuntimes(r.runtimes.filter((id) => id !== r.default));
      })
      .catch(() => {
        /* a deny/unavailable just means the card stays hidden — the manual wizard is the fallback */
      });
    return () => {
      alive = false;
    };
  }, []);
  return runtimes;
}

export function GenerateWithAgent({ ws, onOpenRun }: Props) {
  const runtimes = useExternalRuntimes();
  const [goal, setGoal] = useState("");
  const [runtime, setRuntime] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<AgentResult | null>(null);

  // Auto-select the first external runtime when the list loads.
  useEffect(() => {
    if (runtimes.length > 0 && !runtime) setRuntime(runtimes[0]);
  }, [runtimes, runtime]);

  // Hide when the feature is off (no external runtimes registered) — the card is the door to the
  // external-agent bridge; without it there's nothing to generate with. The manual wizard stays.
  if (runtimes.length === 0) return null;

  const run = async () => {
    if (!goal.trim() || loading) return;
    setLoading(true);
    setError(null);
    try {
      const r = await invokeAgent(ws, crypto.randomUUID(), goal, {
        persona: PERSONA,
        runtime: runtime || undefined,
      });
      setResult(r);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  const submitted = result ? (
    <div className="mt-3 rounded-md border border-border bg-panel px-3 py-2 text-xs text-muted">
      <div className="flex items-center gap-2">
        <Sparkles size={13} className="text-accent" />
        <span>Run started</span>
        {onOpenRun && (
          <button
            type="button"
            onClick={() => onOpenRun(result.jobId)}
            className="ml-auto text-accent hover:underline"
          >
            Open run feed →
          </button>
        )}
      </div>
    </div>
  ) : null;

  return (
    <div className="mb-4 rounded-lg border border-border bg-panel p-4">
      <div className="mb-2 flex items-center gap-2">
        <Bot size={16} className="text-accent" />
        <h3 className="text-sm font-semibold text-fg">Generate with agent</h3>
        <span className="ml-auto rounded-full bg-muted-bg px-2 py-0.5 text-[10px] text-muted">
          {PERSONA}
        </span>
      </div>
      <p className="mb-3 text-xs text-muted">
        Describe what you want — the extension-builder agent scaffolds, builds (container), and
        proposes the publish. You approve in the dock before anything installs.
      </p>
      <div className="flex gap-2">
        <input
          type="text"
          value={goal}
          onChange={(e) => setGoal(e.target.value)}
          placeholder="e.g. build me an energy dashboard page — kWh by hour"
          className="min-w-0 flex-1 rounded-md border border-border bg-bg px-3 py-1.5 text-sm text-fg placeholder:text-muted"
          onKeyDown={(e) => {
            if (e.key === "Enter" && goal.trim() && !loading) run();
          }}
        />
        <select
          value={runtime}
          onChange={(e) => setRuntime(e.target.value)}
          className="rounded-md border border-border bg-bg px-2 py-1.5 text-xs text-fg"
        >
          {runtimes.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>
        <Button
          type="button"
          size="sm"
          onClick={run}
          disabled={!goal.trim() || loading}
          className="gap-1.5"
        >
          {loading ? <Loader2 size={14} className="animate-spin" /> : <Sparkles size={14} />}
          Generate
        </Button>
      </div>
      {error ? (
        <div className="mt-2 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-1.5 text-xs text-destructive">
          {error}
        </div>
      ) : null}
      {submitted}
    </div>
  );
}
