// ResultBar — the status header over the run-result region (rules-editor-ux scope). Turns the bare
// scroll box under the editor into a resolved surface: a run-state dot + label ("Ready" / "Running…" /
// "Failed" / a result summary like "3 rows · 4 ms"), so an author who clicks Run gets unmistakable
// feedback that it ran and what it returned. Reads the already-typed `RunResult`; it computes no data,
// only summarises. One component per file (FILE-LAYOUT).

import type { RunResult } from "@/lib/rules";

type Status = "idle" | "running" | "error" | "ok";
export type ResultView = "table" | "json";

interface ResultBarProps {
  result: RunResult | null;
  error: string | null;
  running: boolean;
  hasRun: boolean;
  /** The current result view + its setter — the Table|JSON toggle lives here (only shown with a result). */
  view: ResultView;
  onViewChange: (view: ResultView) => void;
}

/** A one-line human summary of what a successful run returned, per output kind. */
function summarise(result: RunResult): string {
  const { output } = result;
  switch (output.kind) {
    case "grid": {
      const n = output.rows.length;
      return `${n} ${n === 1 ? "row" : "rows"}`;
    }
    case "scalar":
      return "1 value";
    case "findings": {
      const n = result.findings.length;
      return `${n} ${n === 1 ? "finding" : "findings"}`;
    }
    case "nothing":
      return "no output";
  }
}

export function ResultBar({
  result,
  error,
  running,
  hasRun,
  view,
  onViewChange,
}: ResultBarProps) {
  const status: Status = running ? "running" : error ? "error" : result ? "ok" : "idle";

  const dot = {
    idle: "bg-muted/50",
    running: "bg-accent animate-pulse",
    error: "bg-destructive",
    ok: "bg-emerald-500",
  }[status];

  const label = {
    idle: hasRun ? "Ready" : "Not run yet",
    running: "Running…",
    error: "Failed",
    ok: result ? summarise(result) : "Done",
  }[status];

  return (
    <div
      aria-label="result status"
      className="flex items-center gap-2 border-b border-border bg-panel/60 px-3 py-1.5 text-xs"
    >
      <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${dot}`} aria-hidden />
      <span className="font-medium text-fg">Result</span>
      <span className="text-muted" aria-label="result summary">
        {label}
      </span>
      {status === "ok" && result ? (
        <div className="ml-auto flex items-center gap-3">
          <span className="tabular-nums text-muted" aria-label="result timing">
            {result.ms} ms
          </span>
          {/* Table | JSON — a segmented toggle so the raw shape is one click away (product register:
              standard affordance, accent marks the active segment only). */}
          <div
            role="group"
            aria-label="result view toggle"
            className="flex overflow-hidden rounded-md border border-border"
          >
            {(["table", "json"] as const).map((v) => (
              <button
                key={v}
                type="button"
                aria-label={`view ${v}`}
                aria-pressed={view === v}
                onClick={() => onViewChange(v)}
                className={`px-2 py-0.5 text-[11px] font-medium capitalize transition-colors ${
                  view === v
                    ? "bg-accent/15 text-accent"
                    : "bg-transparent text-muted hover:bg-panel hover:text-fg"
                }`}
              >
                {v}
              </button>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}
