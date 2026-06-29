// RunResult — the result pane: switch on `output.kind` → ScalarCard | GridTable | FindingsList, plus
// the log + budget; OR render the typed error HONESTLY (rules-workbench scope, the headline). A run
// that hit the wall returns a typed error the page renders as itself — a denied source / cage / AI-
// budget / AI-not-configured message, NEVER a fake result. The `error` is the verbatim gateway body: a
// 403 is the generic "not permitted" (opaque), a 400 is the verbatim author feedback. One component.

import type { RunResult as RunResultData } from "@/lib/rules";
import { ScalarCard } from "./ScalarCard";
import { GridTable } from "./GridTable";
import { FindingsList } from "./FindingsList";
import { LogPanel } from "./LogPanel";
import { BudgetBadge } from "./BudgetBadge";

interface RunResultProps {
  result: RunResultData | null;
  error: string | null;
  running: boolean;
}

export function RunResult({ result, error, running }: RunResultProps) {
  if (running) {
    return (
      <div aria-label="run running" className="text-sm text-muted">
        Running…
      </div>
    );
  }

  // The honest failure state — render the typed error as itself, never a fake result.
  if (error) {
    return (
      <div
        aria-label="run error"
        role="alert"
        className="rounded border border-red-300 bg-red-50 p-3 text-sm text-red-800"
      >
        {error}
      </div>
    );
  }

  if (!result) {
    return (
      <div aria-label="run empty" className="text-sm text-muted">
        Run a rule to see its result.
      </div>
    );
  }

  return (
    <div aria-label="run result" className="space-y-3">
      {renderOutput(result)}
      <FindingsList findings={result.findings} />
      <LogPanel log={result.log} />
      <BudgetBadge ms={result.ms} ai={result.ai} />
    </div>
  );
}

function renderOutput(result: RunResultData) {
  const { output } = result;
  switch (output.kind) {
    case "scalar":
      return <ScalarCard value={output.value} />;
    case "grid":
      return <GridTable columns={output.columns} rows={output.rows} />;
    case "findings":
      // The findings ARE the result — rendered by the sibling FindingsList below.
      return null;
    case "nothing":
      return (
        <div aria-label="run nothing" className="text-sm text-muted">
          No output (the rule emitted nothing).
        </div>
      );
  }
}
