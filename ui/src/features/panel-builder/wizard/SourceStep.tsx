// SourceStep (panel-wizard scope) — the wizard's first step. Two tracks, both 100% reused surfaces:
//   - series/native: the SHIPPED source picker (`useSourcePicker` + `SourceCombobox`) the Query tab
//     mounts — the same ws-scoped entries, no second source surface;
//   - datasource (federation): pick a registered datasource and author against it through the FULL
//     `QueryWorkbench` — the exact page the Datasources detail mounts (Builder⇄Code, canvas, Run,
//     history, SAVED QUERIES). Running a query (or loading a saved SQL one) adopts it as the panel's
//     source via the workbench's `onUseSql` seam: the target becomes `federation.query {source, sql}`
//     (the same wire shape the editor's Query tab writes) — prove the SQL by running it.
//
// No wizard-only state: the chosen track lives entirely in `state.targets[0]` (+ `state.sql` for the
// authored query); the saved-query selection (`sel`) is presentation-only dialog state.
//
// One responsibility: pick a read source into the wizard's primary target.

import { useState } from "react";

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import type { Target } from "@/lib/dashboard";
import { useSourcePicker } from "@/features/dashboard/builder/useSourcePicker";
import { READ_SOURCE_GROUPS, SourceCombobox, type SourceEntry, SQL_SOURCE_ID } from "@/features/dashboard/builder/sourcePicker";
import { Select } from "@/components/ui/select";
import { defaultOptionsForView } from "@/features/panel-builder/viewOptions";
import { useDatasourceList, refForOption } from "@/features/panel-builder/tabs/useDatasourceList";
import { QueryWorkbench } from "@/features/query-workbench/QueryWorkbench";

interface Props {
  ws: string;
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
}

/** Build the primary target from a chosen picker entry (mirrors QueryTab.targetFromEntry). */
function targetFromEntry(entry: SourceEntry | null, prev: Target | undefined): Target {
  const refId = prev?.refId || "A";
  if (!entry || !entry.source) return { refId, tool: "", args: {}, datasource: { type: "surreal" } };
  const tool = entry.source.tool;
  return {
    refId,
    tool,
    args: (entry.source.args as Record<string, unknown>) ?? {},
    datasource: { type: tool === "store.query" ? "surreal" : "series" },
  };
}

export function SourceStep({ ws, state, patch }: Props) {
  // EAGER: the wizard's whole purpose is to pick a source, so the picker loads on mount (unlike the
  // editor's Query tab, which waits for focus — the wizard IS the focused surface).
  const { entries, loading } = useSourcePicker(ws, { enabled: true });
  const { options: dsOptions, loading: dsLoading } = useDatasourceList(ws);
  const fedOptions = dsOptions.filter((o) => o.type === "federation");
  const primary = state.targets[0];

  // The datasource track is DERIVED from the target (no second source of truth): a federation.query
  // target names its datasource in args.source.
  const fedSource = primary?.tool === "federation.query" ? ((primary.args?.source as string) ?? "") : "";
  const fedSql = primary?.tool === "federation.query" ? ((primary.args?.sql as string) ?? "") : "";
  // The saved-query dialog selection — presentation-only (the adopted SQL is what persists).
  const [sel, setSel] = useState<string | null>(null);

  /** The picker entry id matching the current target (so the combobox reopens showing the picked source). */
  const pickedEntryId = primary?.tool
    ? entries.find((e) => {
        if (e.source?.tool !== primary.tool) return false;
        const pickedSeries = (primary.args as { series?: string } | undefined)?.series;
        if (!pickedSeries) return true;
        return (e.source.args as { series?: string } | undefined)?.series === pickedSeries;
      })?.id ?? ""
    : "";

  const selectEntry = (entry: SourceEntry | null) => {
    if (entry?.id === SQL_SOURCE_ID) {
      // The SQL source — empty SQL on first pick; the editor's Query tab is where the query is authored
      // (the wizard's simple track picks a labeled source). A user wanting SQL uses the editor.
      patch({
        sql: { mode: "code", rawSql: "", format: "table" },
        targets: [{ ...targetFromEntry(entry, primary), tool: "store.query", args: { sql: "" } }],
      });
      return;
    }
    patch({ sql: undefined, targets: [targetFromEntry(entry, primary)] });
  };

  /** Switch to the datasource track: a `federation.query` target with empty SQL — the embedded
   *  workbench authors it (mirrors QueryTab.selectDatasource's federation branch). */
  const selectDatasource = (name: string) => {
    setSel(null);
    if (!name) {
      patch({ sql: undefined, targets: [{ refId: primary?.refId || "A", tool: "", args: {}, datasource: { type: "surreal" } }] });
      return;
    }
    const opt = fedOptions.find((o) => o.name === name);
    const ds = opt ? refForOption(opt, ws) : { type: "federation" };
    patch({
      // SQL rows are arbitrary-shaped — default the preview to TABLE so the adopted query's rows show
      // immediately (a timeseries can't shape a summary result → an honest-but-unhelpful "no data
      // yet"). The chart-type step (step 2) is where the user picks the viz for chartable shapes.
      view: "table",
      options: defaultOptionsForView("table"),
      sql: { mode: "code", rawSql: "", format: "table" },
      targets: [{ refId: primary?.refId || "A", tool: "federation.query", args: { source: name, sql: "" }, datasource: ds }],
    });
  };

  /** Adopt the SQL the workbench just ran / loaded as the panel's source (the `onUseSql` seam). */
  const adoptSql = (sql: string) => {
    if (!fedSource) return;
    patch({
      sql: { mode: "code", rawSql: sql, format: "table" },
      targets: [{ ...(primary as Target), args: { source: fedSource, sql } }],
    });
  };

  return (
    <div className="grid gap-3" aria-label="wizard source step">
      <div className="grid gap-1">
        <h2 className="text-sm font-medium text-fg">Pick a source</h2>
        <p className="text-xs text-muted">
          A workspace read source (series, SQL, queries, rules) — or a registered datasource with its
          full query editor and saved queries.
        </p>
      </div>
      <label className="grid gap-1 text-xs text-muted">
        Source
        <SourceCombobox
          aria-label="wizard source"
          entries={entries}
          value={fedSource ? "" : pickedEntryId}
          loading={loading}
          groups={READ_SOURCE_GROUPS.filter(({ group }) => group !== "flows")}
          onSelect={() => {}}
          onSelectEntry={(e) => selectEntry(e ?? null)}
        />
      </label>
      {fedOptions.length > 0 && (
        <label className="grid gap-1 text-xs text-muted">
          Datasource
          <Select
            aria-label="wizard datasource"
            className="h-8 w-full"
            value={fedSource}
            disabled={dsLoading}
            onChange={(e) => selectDatasource(e.target.value)}
          >
            <option value="">— none (use a source above) —</option>
            {fedOptions.map((o) => (
              <option key={o.name} value={o.name}>
                {o.label}
              </option>
            ))}
          </Select>
        </label>
      )}
      {fedSource && (
        <div className="grid gap-1.5">
          <p className="text-[11px] text-muted">
            Author against <code className="text-fg">{fedSource}</code> — Run a query (or load a saved
            one) to use it as the panel's source.
          </p>
          <div className="h-[26rem] min-h-0 overflow-hidden rounded-md border border-border" aria-label="wizard datasource workbench">
            {/* `initial={state.sql}` — the wizard REMOUNTS this on Back/Next; seeding from the
                persisted EditorState keeps the authored query instead of resetting the editor. */}
            <QueryWorkbench ws={ws} source={fedSource} sel={sel} onSel={setSel} onUseSql={adoptSql} initial={state.sql} />
          </div>
        </div>
      )}
      {primary?.tool && (
        <p className="text-[11px] text-muted" aria-label="wizard source picked">
          picked:{" "}
          <code className="text-fg">
            {primary.tool}
            {primary.args && primary.args.series ? ` → ${primary.args.series}` : ""}
            {fedSource ? ` → ${fedSource}${fedSql ? ` · ${fedSql.slice(0, 60)}${fedSql.length > 60 ? "…" : ""}` : " (run a query to bind it)"}` : ""}
          </code>
        </p>
      )}
    </div>
  );
}
