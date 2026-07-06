// The reusable Query workbench (query-workbench-view scope, slice 3) — the full author + run + see
// surface that mounts in three homes: the Datasources detail page (federation, standard dialect),
// the Data page (pinned to `surreal-local`, surreal dialect), and a Data Studio pane (the same
// component, surreal-local default). One component, three homes, one model — the persisted
// `SqlSourceState` round-trips byte-for-byte as it does today (`cellEditorState.ts`).
//
// The workbench is the no-chrome core: it owns the editor state, the run dispatch, the results
// area, and (for a federation source) the saved-query dialogs. The host page supplies the page
// header / rail / probe chrome. The `source` prop is opaque config data (rule 10): `"surreal-local"`
// selects the surreal dialect + `store.query`/`store.schema`; any other string selects the standard
// dialect + `federation.query`/`federation.schema` for that named datasource. The workspace is the
// hard wall (rule 6) — neither engine takes a workspace arg; both pin from the token.
//
// Slice 1 (canvas builder) + slice 2 (schema-aware editor) plug in unchanged: the workbench hosts
// `<SqlQueryEditor>`, which switches its Builder body by dialect (canvas for standard-with-schema,
// rows for surreal/empty) and its Code completion by the schema feed — both fed by the same
// `useLocalSchema` / `useFederationSchema` hooks the panel-builder's QueryTab uses (the scope's
// "reuse that composition, don't re-derive it"). Reusing a saved query (`sel`) goes through the
// SHIPPED `query.*` verbs via `useDatasourceQueries` — no new persistence (the `querydef.*` chain
// is dead; the umbrella §"Saved queries" closed that open item).

import { useEffect, useState } from "react";
import { Loader2, Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import { useFederationSchema } from "@/features/panel-builder/tabs/useFederationSchema";
import { useLocalSchema } from "@/features/panel-builder/tabs/useLocalSchema";
import { SqlQueryEditor } from "@/features/dashboard/builder/sql/SqlQueryEditor";
import { QueryResults } from "@/features/datasources/QueryResults";
import { SaveQueryDialog } from "@/features/datasources/SaveQueryDialog";
import { SavedQueriesDialog } from "@/features/datasources/SavedQueriesDialog";
import { useDatasourceQueries } from "@/features/datasources/useDatasourceQueries";
import type { QuerySummary } from "@/lib/queries";
import { emptyQuery, emptySqlSource, type SqlSourceState } from "@/lib/panel-kit/sql/query";
import { emitSql, type SqlDialect } from "@/lib/panel-kit/sql/dialect";
import { SURREAL_LOCAL, useQueryRun } from "./useQueryRun";

export interface QueryWorkbenchProps {
  /** The workspace handle (display / deep-link only — no data call takes a workspace arg; the host
   *  pins the workspace from the session token). */
  ws: string;
  /** The current saved-query id (a `query:{ws}:{id}` record targeting `datasource:<source>`), or
   *  null for a fresh unsaved builder. Resolved via `query.get` on change. */
  sel: string | null;
  /** Update the persisted selection (the host's deep-link / pane-param writer). */
  onSel: (id: string | null) => void;
  /** The source to query. `"surreal-local"` ⇒ surreal dialect + `store.query`/`store.schema`; any
   *  other string ⇒ the federation datasource of that name (standard dialect). Config data, never
   *  an extension id (rule 10). */
  source: string;
}

/** The Query workbench — editor + run bar + results + (federation only) saved queries. */
export function QueryWorkbench({ ws, sel, onSel, source }: QueryWorkbenchProps) {
  const isSurreal = source === SURREAL_LOCAL;
  const dialect: SqlDialect = isSurreal ? "surreal" : "standard";

  const [state, setState] = useState<SqlSourceState>(emptySqlSource());
  // Track the builder's selected table so the federation schema hook lazy-fills its columns (the
  // same coupling QueryTab has; the editor is transport-agnostic, the host owns the load).
  const [selectedTable, setSelectedTable] = useState<string>("");

  // The schema feed — same composition the panel-builder's QueryTab uses (scope: "reuse, don't
  // re-derive"). Both hooks early-return when their branch is inactive; a deny/empty load collapses
  // to `tables: []` (the system-catalog deny contract — the Code half still works, dropdowns empty).
  const localSchema = useLocalSchema(isSurreal);
  const federationSchema = useFederationSchema(
    isSurreal ? null : source,
    selectedTable,
    !isSurreal,
  );
  const schema = isSurreal ? localSchema : federationSchema;

  const run = useQueryRun(source);
  // Saved queries target a DATASOURCE (a `datasource:<name>` record). The surreal-local store has
  // no datasource row, so saved queries are a federation-only affordance here (the Data page's
  // surreal SQL box is ad-hoc; a `platform:` target is a separate follow-up).
  const saved = useDatasourceQueries(isSurreal ? "" : source);

  // The selected table follows the builder's FROM table so the federation hook describes it.
  useEffect(() => {
    setSelectedTable(state.builder?.table ?? "");
  }, [state.builder?.table]);

  // Resolve a saved-query selection (`sel`) into the editor — loads the saved text as Code mode.
  useEffect(() => {
    if (!sel) return;
    let cancelled = false;
    saved
      .load(sel)
      .then((q) => {
        if (cancelled) return;
        setState({ mode: "code", rawSql: q.text, format: "table" });
      })
      .catch(() => {
        /* a load failure (deny/NotFound) leaves the editor as-is — honest, no fabricated text */
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sel]);

  const editorSql =
    state.mode === "builder"
      ? emitSql(dialect, state.builder ?? emptyQuery())
      : state.rawSql;

  const onRun = () => {
    const sql = editorSql;
    if (sql.trim()) void run.run(sql);
  };

  const onLoadSaved = (q: QuerySummary) => {
    onSel(q.id);
    // The sel→load effect above resolves the full record and loads the text.
  };

  return (
    <div className="flex h-full min-h-0 flex-col" aria-label="query workbench" data-ws={ws}>
      <div className="min-h-0 flex-1 overflow-hidden">
        <SqlQueryEditor
          dialect={dialect}
          schema={schema}
          value={state}
          onChange={setState}
        />
      </div>

      <div className="flex flex-wrap items-center gap-1.5 border-t border-border bg-panel/40 px-3 py-1.5">
        <Button
          aria-label="run query"
          size="sm"
          variant="solid"
          className="gap-1.5"
          onClick={onRun}
          disabled={run.loading || !editorSql.trim()}
        >
          {run.loading ? <Loader2 size={13} className="animate-spin" /> : <Play size={13} />} Run
        </Button>
        {/* Saved queries are a federation-only affordance (they target datasource:<name>). */}
        {!isSurreal && (
          <>
            <SavedQueriesDialog
              queries={saved.queries}
              loading={saved.loading}
              error={saved.error}
              onLoad={onLoadSaved}
              onDelete={(id) => saved.remove(id)}
            />
            <SaveQueryDialog
              source={source}
              sql={editorSql}
              disabled={!editorSql.trim()}
              onSave={(args) => saved.save(args).then((id) => { onSel(id); return id; })}
            />
          </>
        )}
        {run.lastSql && !run.loading && (
          <span className="ml-auto truncate text-[11px] text-muted" title={run.lastSql}>
            {run.result
              ? `${run.result.rows.length} row${run.result.rows.length === 1 ? "" : "s"} · ${run.result.columns.length} col${run.result.columns.length === 1 ? "" : "s"}`
              : "ran"}
          </span>
        )}
        {run.error && (
          <span
            role="alert"
            className="ml-auto truncate text-[11px] text-destructive"
            title={run.error}
          >
            {run.error}
          </span>
        )}
      </div>

      <div className="min-h-[8rem] flex-1 border-t border-border">
        <QueryResults
          result={run.result}
          emptyHint={
            run.error
              ? run.error
              : "Write a SELECT and hit Run (⌘/Ctrl+Enter). A write/multi statement is refused at the host."
          }
        />
      </div>
    </div>
  );
}
