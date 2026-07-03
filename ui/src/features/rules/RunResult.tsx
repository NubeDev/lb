// RunResult — the result pane: switch on `output.kind` → ScalarCard | GridTable | FindingsList, plus
// the log + budget; OR render the typed error HONESTLY (rules-workbench scope, the headline). A run
// that hit the wall returns a typed error the page renders as itself — a denied source / cage / AI-
// budget / AI-not-configured message, NEVER a fake result. The `error` is the verbatim gateway body: a
// 403 is the generic "not permitted" (opaque), a 400 is the verbatim author feedback. One component.

import type { RunResult as RunResultData } from "@/lib/rules";
import type { ResultView } from "./ResultBar";
import { ScalarCard } from "./ScalarCard";
import { GridTable } from "./GridTable";
import { JsonView } from "./JsonView";
import { FindingsList } from "./FindingsList";
import { LogPanel } from "./LogPanel";
import { BudgetBadge } from "./BudgetBadge";

interface RunResultProps {
  result: RunResultData | null;
  error: string | null;
  running: boolean;
  /** Whether a run has completed — distinguishes the first-load idle state from a finished empty run. */
  hasRun: boolean;
  /** Table (typed views) or JSON (verbatim result). Only meaningful once there's a result. */
  view: ResultView;
}

export function RunResult({ result, error, running, hasRun, view }: RunResultProps) {
  if (running) {
    // A skeleton, not a spinner-in-content (product register): the shape the result will take.
    return (
      <div aria-label="run running" className="space-y-2">
        <div className="h-4 w-40 animate-pulse rounded bg-muted/25" />
        <div className="h-4 w-full animate-pulse rounded bg-muted/20" />
        <div className="h-4 w-3/4 animate-pulse rounded bg-muted/15" />
      </div>
    );
  }

  // The honest failure state — render the typed error as itself, never a fake result. Uses the shared
  // destructive token (not raw red-*), so the workbench speaks the same state vocabulary as every page.
  if (error) {
    return (
      <div
        aria-label="run error"
        role="alert"
        className="rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
      >
        <div className="mb-1 text-xs font-semibold uppercase tracking-wide">Rule failed</div>
        <p className="font-mono text-[13px] leading-relaxed">{error}</p>
      </div>
    );
  }

  if (!result) {
    return (
      <div aria-label="run empty" className="text-sm text-muted">
        {hasRun
          ? "The rule ran but returned no result."
          : "Run a rule to see its result — press Run or ⌘↵."}
      </div>
    );
  }

  // JSON view: the verbatim result, one click from the typed view (still with log + budget below).
  if (view === "json") {
    return (
      <div aria-label="run result" className="space-y-3">
        <JsonView result={result} />
        <LogPanel log={result.log} />
        <BudgetBadge ms={result.ms} ai={result.ai} />
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
