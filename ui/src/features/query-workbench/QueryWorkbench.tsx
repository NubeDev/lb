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
// The editor↔results split (drag handle between the run bar and results + maximise/restore buttons
// in the run bar) is owned by `useWorkbenchSplit`. The editor section is a flex column bounded by the
// split; inside it, the Canvas fills all available height (maximise the editor → the React-Flow
// surface grows), and the Rules form scrolls within its own wrapper when it overflows.
//
// Slice 1 (canvas builder) + slice 2 (schema-aware editor) plug in unchanged: the workbench hosts
// `<SqlQueryEditor>`, which switches its Builder body by dialect (canvas for standard-with-schema,
// rows for surreal/empty) and its Code completion by the schema feed — both fed by the same
// `useLocalSchema` / `useFederationSchema` hooks the panel-builder's QueryTab uses (the scope's
// "reuse that composition, don't re-derive it"). Reusing a saved query (`sel`) goes through the
// SHIPPED `query.*` verbs via `useDatasourceQueries` — no new persistence (the `querydef.*` chain
// is dead; the umbrella §"Saved queries" closed that open item).

import { useEffect, useRef, useState } from "react";

import { compileQuery } from "@/lib/queries";
import { Loader2, Maximize2, Minimize2, Play } from "lucide-react";

import { Button } from "@/components/ui/button";
import { formatMs } from "@/lib/format/formatMs";
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
import { RunHistoryMenu } from "./RunHistoryMenu";
import { loadHistory, recordRun, type RunHistoryEntry } from "./runHistory";
import { useQueryDraftFollow } from "./useQueryDraftFollow";
import { useWorkbenchSplit } from "./useWorkbenchSplit";

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
  /** Optional adoption seam for hosts that BIND the authored query somewhere (the panel wizard's
   *  source step): called with the SQL that actually runs — on Run (for PRQL, the compiled SQL) and
   *  when a saved SQL query is loaded. Absent for the standalone homes (Datasources/Data/Studio). */
  onUseSql?: (sql: string) => void;
  /** Optional initial editor state (first mount only). A binding host that REMOUNTS the workbench
   *  (the wizard's Back/Next) passes its persisted `SqlSourceState` so the authored query survives
   *  the round-trip instead of resetting to an empty editor. */
  initial?: SqlSourceState;
}

/** The Query workbench — editor + run bar + results + (federation only) saved queries. */
export function QueryWorkbench({ ws, sel, onSel, source, onUseSql, initial }: QueryWorkbenchProps) {
  const isSurreal = source === SURREAL_LOCAL;
  const dialect: SqlDialect = isSurreal ? "surreal" : "standard";

  const [state, setState] = useState<SqlSourceState>(() => initial ?? emptySqlSource());
  const rootRef = useRef<HTMLDivElement>(null);
  const split = useWorkbenchSplit(rootRef);
  // Every table the builder references — the FROM table plus each canvas-joined table — so the
  // federation schema hook lazy-fills ALL their columns (a canvas-added join table must load its
  // columns too, not just the FROM table).
  const builderTables = [
    state.builder?.table ?? "",
    ...(state.builder?.joins ?? []).map((j) => j.table),
  ].filter(Boolean);

  // The schema feed — same composition the panel-builder's QueryTab uses (scope: "reuse, don't
  // re-derive"). Both hooks early-return when their branch is inactive; a deny/empty load collapses
  // to `tables: []` (the system-catalog deny contract — the Code half still works, dropdowns empty).
  const localSchema = useLocalSchema(isSurreal);
  const federationSchema = useFederationSchema(
    isSurreal ? null : source,
    builderTables,
    !isSurreal,
  );
  const schema = isSurreal ? localSchema : federationSchema;

  const run = useQueryRun(source);
  // Follow live agent-authored draft frames (query-draft-streaming scope): each valid frame
  // REPLACES the editor state — the canvas/rules/code bodies all re-derive from it. A user edit
  // after a frame simply wins locally (last writer; no co-editing semantics in v1).
  const lastDraftAt = useQueryDraftFollow(source, setState);
  // Saved queries target a DATASOURCE (a `datasource:<name>` record). The surreal-local store has
  // no datasource row, so saved queries are a federation-only affordance here (the Data page's
  // surreal SQL box is ad-hoc; a `platform:` target is a separate follow-up).
  const saved = useDatasourceQueries(isSurreal ? "" : source);

  // Resolve a saved-query selection (`sel`) into the editor — loads the saved text as Code mode,
  // restoring the saved language (a `lang:"prql"` record reopens with the PRQL toggle set).
  useEffect(() => {
    if (!sel) return;
    let cancelled = false;
    saved
      .load(sel)
      .then((q) => {
        if (cancelled) return;
        setState({
          mode: "code",
          rawSql: q.text,
          format: "table",
          lang: q.lang === "prql" ? "prql" : "sql",
        });
        // A loaded SQL saved query is directly runnable — hand it to a binding host. PRQL text must
        // go through Run (compile) first, so it is adopted there with the compiled SQL.
        if (q.lang !== "prql") onUseSql?.(q.text);
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

  // The last 10 UNIQUE runs against this source (per workspace, localStorage) — restore drops the
  // SQL back into Code mode. Reloaded when the source changes.
  const [history, setHistory] = useState<RunHistoryEntry[]>(() => loadHistory(ws, source));
  useEffect(() => {
    setHistory(loadHistory(ws, source));
  }, [ws, source]);

  // PRQL is active only in Code mode on a federation source (Builder always emits SQL; PRQL has no
  // SurrealQL backend). The PRQL text compiles server-side (`query.compile`, the same `lb-prql`
  // path saved `lang:"prql"` queries run through) and the COMPILED SQL is what runs — so the status
  // bar's `lastSql` shows the real statement the engine saw.
  const prqlActive = !isSurreal && state.mode === "code" && state.lang === "prql";
  const [compileError, setCompileError] = useState<string | null>(null);

  const onRun = () => {
    const text = editorSql;
    if (!text.trim()) return;
    setHistory(recordRun(ws, source, text, Date.now()));
    setCompileError(null);
    if (!prqlActive) {
      onUseSql?.(text);
      void run.run(text);
      return;
    }
    void compileQuery({ lang: "prql", text, target: `datasource:${source}` })
      .then((res) => {
        onUseSql?.(res.sql);
        return run.run(res.sql);
      })
      .catch((e) => setCompileError(e instanceof Error ? e.message : String(e)));
  };

  const onRestoreHistory = (sql: string) =>
    setState((s) => ({ ...s, mode: "code", rawSql: sql, builder: undefined }));

  const onLoadSaved = (q: QuerySummary) => {
    onSel(q.id);
    // The sel→load effect above resolves the full record and loads the text.
  };

  return (
    <div ref={rootRef} className="flex h-full min-h-0 flex-col" aria-label="query workbench" data-ws={ws}>
      {/* Editor — bounded by the split hook's flex height. `overflow-hidden` here; scrolling for the
          Rules form happens inside VisualEditor's Rules wrapper, and the Canvas fills the space (no
          scroll). The flex column lets SqlQueryEditor fill the available height so the Canvas grows
          when the editor section is maximised. */}
      <div
        className="flex min-h-0 flex-col overflow-hidden px-3 pb-2"
        style={split.editorStyle}
      >
        <SqlQueryEditor
          dialect={dialect}
          schema={schema}
          value={state}
          onChange={setState}
          allowPrql={!isSurreal}
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
        <RunHistoryMenu entries={history} onRestore={onRestoreHistory} />
        {lastDraftAt !== null && (
          <span
            aria-label="live draft indicator"
            title="An agent is streaming this query (last frame applied to the editor)"
            className="flex items-center gap-1 rounded-md bg-accent/10 px-1.5 py-0.5 text-[10px] font-medium text-accent"
          >
            <span className="h-1.5 w-1.5 animate-pulse rounded-full bg-accent" />
            live draft
          </span>
        )}
        {/* Saved queries are a federation-only affordance (they target datasource:<name>). */}
        {!isSurreal && (
          <>
            <SavedQueriesDialog
              queries={saved.queries}
              loading={saved.loading}
              error={saved.error}
              onLoad={onLoadSaved}
              onDelete={(id) => saved.remove(id)}
              onFetchText={(id) => saved.load(id).then((q) => q.text)}
            />
            <SaveQueryDialog
              source={source}
              sql={editorSql}
              disabled={!editorSql.trim()}
              onSave={(args) =>
                saved
                  // A PRQL draft saves as a real `lang:"prql"` record (the host compiles at run);
                  // everything else stays `raw` (the shipped default).
                  .save({ ...args, lang: prqlActive ? "prql" : "raw" })
                  .then((id) => { onSel(id); return id; })
              }
            />
          </>
        )}
        {run.lastSql && !run.loading && (
          <span className="ml-auto truncate text-[11px] text-muted" title={run.lastSql}>
            {run.result
              ? `${run.result.rows.length} row${run.result.rows.length === 1 ? "" : "s"} · ${run.result.columns.length} col${run.result.columns.length === 1 ? "" : "s"}${
                  formatMs(run.elapsedMs) ? ` · ${formatMs(run.elapsedMs)}` : ""
                }`
              : "ran"}
          </span>
        )}
        {run.error && !compileError && (
          <span
            role="alert"
            className="ml-auto truncate text-[11px] text-destructive"
            title={run.error}
          >
            {run.error}
          </span>
        )}
        {/* A PRQL compile failure is author feedback from `query.compile` — shown in the same slot. */}
        {compileError && (
          <span
            role="alert"
            className="ml-auto truncate text-[11px] text-destructive"
            title={compileError}
          >
            {compileError}
          </span>
        )}
        {/* Maximise / restore the editor and results sections (the split hook owns the ratio). */}
        <div className={run.lastSql || run.error ? "" : "ml-auto flex items-center gap-0.5"}>
          <Button
            aria-label={split.editorMaximised ? "restore editor" : "maximise editor"}
            title={split.editorMaximised ? "Restore split" : "Maximise editor"}
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-muted"
            onClick={split.toggleEditor}
          >
            {split.editorMaximised ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
          </Button>
          <Button
            aria-label={split.resultsMaximised ? "restore results" : "maximise results"}
            title={split.resultsMaximised ? "Restore split" : "Maximise results"}
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-muted"
            onClick={split.toggleResults}
          >
            {split.resultsMaximised ? <Minimize2 size={13} /> : <Maximize2 size={13} />}
          </Button>
        </div>
      </div>

      {/* Drag handle — drag up/down to resize the editor vs results split. */}
      <div
        {...split.dividerProps}
        className="group flex h-1.5 cursor-row-resize items-center justify-center border-y border-border bg-panel/30 hover:bg-accent/10"
      >
        <div className="h-0.5 w-10 rounded-full bg-border transition-colors group-hover:bg-accent/40" />
      </div>

      <div className="min-h-0 border-b border-border" style={split.resultsStyle}>
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
