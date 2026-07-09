// Shared setup-wizard step: run a preloaded, read-only SQL query (setup scope, rule-3 extraction).
// This is the SQL step BOTH the data→insight wizard and the render-template wizard use — one
// implementation, two homes (setup-wizards-scope "extract, don't fork"). It reuses the Query
// workbench's real `useQueryRun` dispatch + `QueryResults` grid verbatim; the query is PRELOADED in a
// read-only `CodeEditor` (a guided run, not an authoring surface — the Query workbench owns authoring).
//
// The render-template wizard needs the RAN ROWS (to feed the AI prompt + the widget preview), so this
// step optionally reports them via `onRows` on each successful run. The data→insight wizard omits it.
//
// One responsibility per file (FILE-LAYOUT): the read-only SQL preview + run.

import { useEffect } from "react";
import { Loader2, Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import { CodeEditor, codeLanguageExtension } from "@/components/codeeditor";
import { QueryResults } from "@/features/datasources/QueryResults";
import { useQueryRun } from "@/features/query-workbench/useQueryRun";
import { DEMO_SQL } from "../dataToInsight";

// One CodeMirror language extension — resolved once (a stable array keeps CodeMirror from
// re-configuring on every render).
const SQL_EXT = [codeLanguageExtension("sql")!];

interface Props {
  source: string;
  /** Cap-gate the Run button (display only — the gateway re-checks `federation.query`). */
  canRun: boolean;
  /** The SQL to preview + run. Defaults to the shipped buildings-demo query. */
  sql?: string;
  /** Report the ran rows on each successful run (the render-template wizard feeds them to the AI
   *  prompt + the widget preview). Omit for a run-and-see step that needs no rows downstream. */
  onRows?: (rows: Record<string, unknown>[]) => void;
}

/** The read-only SQL preview + run body — reuses the workbench engine (`useQueryRun`) + `QueryResults`
 *  verbatim. Preloaded and read-only: the user runs, doesn't author (authoring is the Query workbench). */
export function SqlPreviewStep({ source, canRun, sql = DEMO_SQL, onRows }: Props) {
  const run = useQueryRun(source);

  // Report rows up whenever a fresh result lands (the template wizard binds its preview + AI prompt to
  // the user's real query result). Keyed on the result identity so it fires once per run.
  useEffect(() => {
    if (run.result && onRows) onRows(run.result.rows as Record<string, unknown>[]);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- fire on each new result object
  }, [run.result]);

  return (
    <div className="space-y-3">
      <div className="overflow-hidden rounded-md border border-border">
        <CodeEditor value={sql} onChange={() => {}} extensions={SQL_EXT} editable={false} ariaLabel="query" height="auto" />
      </div>

      <div className="flex items-center gap-3">
        <Button
          size="sm"
          disabled={!canRun || run.loading}
          onClick={() => void run.run(sql)}
          aria-label="Run the query"
        >
          {run.loading ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} />}
          {run.loading ? "Running…" : "Run against " + source}
        </Button>
        {run.elapsedMs != null && !run.loading && (
          <span className="text-xs text-muted">{run.result?.rows.length ?? 0} rows · {run.elapsedMs} ms</span>
        )}
        {!canRun && <span className="text-xs text-muted">You don&apos;t have query access here.</span>}
      </div>

      {run.error && (
        <p role="alert" className="rounded-md border border-destructive/30 bg-destructive/10 p-2 text-xs text-destructive">
          {run.error}
        </p>
      )}
      {run.result && <QueryResults result={run.result} emptyHint="The query returned no rows." />}
    </div>
  );
}
