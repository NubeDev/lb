// RuleWorkbench (panel-wizard-source-discoverability scope, slice 2) — the rule track's PROVE-IT loop,
// the parity twin of the datasource track's `QueryWorkbench`. A user who picked a saved rule as the
// panel's source can, right here: fill the rule's declared params (the shipped `RuleParamsSection`),
// press **Run**, and see the returned rows/scalar/error in the shipped `RunResult` pane — so the rule is
// proven to return data BEFORE it's bound, exactly as SQL is proven with Run in the datasource track.
//
// It reuses the REAL run path (`rules.run` via `runRule`) and the REAL result renderers (`RunResult` →
// GridTable/ScalarCard/FindingsList/error) — no second fetch, no re-rolled table. The rule id + params
// live on the picked target (`state.targets[0].args`), the single source of truth; this component holds
// only transient run state (result/error/running), never a copy of the binding.
//
// One responsibility: run the picked rule and show its result, so the author can trust it before Next.

import { useState } from "react";
import { Play } from "lucide-react";

import { runRule, type RunResult as RunResultData } from "@/lib/rules";
import { Button } from "@/components/ui/button";
import type { Target } from "@/lib/dashboard";
import type { RuleParam } from "@nube/source-picker";
import { RuleParamsSection } from "@/features/panel-builder/tabs/RuleParamsSection";
import { RunResult } from "@/features/rules/RunResult";

interface Props {
  /** The picked rule target (`rules.run {rule_id, route:false, params?}`) — the source of truth. */
  target: Target;
  /** The rule's declared params (from the picker entry), so the form is typed. Empty → no form. */
  params: RuleParam[];
  /** Apply the target with edited params (the params form writes `args.params`). */
  onChange: (next: Target) => void;
}

/** The rule id off the target's args (the picker always sets it; guard for the empty transient). */
function ruleIdOf(target: Target): string {
  return (target.args?.rule_id as string | undefined) ?? "";
}

/** The params map off the target's args (what the form edited), defaulting to none. */
function paramsOf(target: Target): Record<string, unknown> | undefined {
  const p = target.args?.params;
  return p && typeof p === "object" ? (p as Record<string, unknown>) : undefined;
}

export function RuleWorkbench({ target, params, onChange }: Props) {
  const [result, setResult] = useState<RunResultData | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);
  const [hasRun, setHasRun] = useState(false);

  const ruleId = ruleIdOf(target);

  const run = async () => {
    if (!ruleId) return;
    setRunning(true);
    setError(null);
    try {
      // The SAME `rules.run` the panel will call at render time (params ride verbatim) — proving the
      // exact bound behaviour, not an approximation. `route:false` on the target keeps it read-only.
      const res = await runRule({ ruleId, params: paramsOf(target) });
      setResult(res);
    } catch (e) {
      // The verbatim gateway body — a 403 (denied source/cage) or 400 (author feedback) shows as itself
      // through RunResult's typed error branch, never a fabricated result.
      setResult(null);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setRunning(false);
      setHasRun(true);
    }
  };

  return (
    <div className="grid gap-2" aria-label="wizard rule workbench">
      {params.length > 0 && (
        <RuleParamsSection params={params} target={target} onChange={onChange} />
      )}
      <div className="flex items-center gap-2">
        <Button
          type="button"
          size="sm"
          aria-label="run rule"
          disabled={running || !ruleId}
          onClick={run}
        >
          <Play size={14} />
          Run
        </Button>
        <span className="text-[11px] text-muted">
          Run the rule to see its rows before binding it — read-only, no findings routed.
        </span>
      </div>
      <RunResult result={result} error={error} running={running} hasRun={hasRun} view="table" />
    </div>
  );
}
