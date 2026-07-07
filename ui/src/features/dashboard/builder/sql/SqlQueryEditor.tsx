// The Grafana-style Builder⇄Code SQL editor (widget-builder Slice C) — ported from Grafana's
// `QueryEditor.tsx`. It switches between the visual `VisualEditor` (Builder) and the raw `RawEditor`
// (Code) by `editorMode`, keeping the two in sync:
//   - Builder edits the typed `SqlBuilderQuery`; every change regenerates the raw SQL via `emitSql`
//     for the editor's `dialect` (Builder is the source of truth in Builder mode).
//   - Code edits the raw string directly (the source of truth in Code mode).
//   - Builder→Code: free (regenerate the string).
//   - Code→Builder: PARSE the raw SQL back into the typed builder query (`parseSql`) — the three
//     views (Code / Rules / Canvas) are projections of ONE model, so the projection runs both ways.
//     Only when the SQL is not expressible in the model (subquery, CTE, window fn, PRQL, …) do we
//     confirm; on confirm the raw SQL is KEPT and the builder starts from the salvaged FROM table.
//
// Dialect-agnostic (query-builder-common scope): the editor takes a `dialect: SqlDialect` (surreal
// for native `store.query`, standard for a federation source) and a `schema: Schema` (the
// table/column dropdown source). The HOST decides both — the editor does not import `readSchema`
// or any federation client; it stays transport-agnostic. The same component serves both datasource
// kinds (no fork, rule 10 — `dialect` is config data, never a hardcoded datasource name).
//
// Builder mode can only ever generate a SELECT; Code mode is still parse-allowlisted to SELECT by
// `store.query` / SELECT-validated by `federation.query` at the host.
//
// Slice 2: passes schema+dialect to RawEditor (schema-aware completion) and wires the Format SQL
// action (button + Cmd/Ctrl+Shift+F keybinding) — gated to Code mode + standard dialect.

import { useCallback, useEffect } from "react";

import { formatSql } from "@/lib/sql/format/sqlFormat";
import type { Schema } from "@/lib/schema";
import { emptyQuery, emptySqlSource, type SqlSourceState } from "@/lib/panel-kit/sql/query";
import { emitSql, type SqlDialect } from "@/lib/panel-kit/sql/dialect";
import { parseSql, salvageFromTable } from "@/lib/panel-kit/sql/parseSql";
import { RawEditor } from "./RawEditor";
import { SqlQueryHeader } from "./SqlQueryHeader";
import { VisualEditor } from "./VisualEditor";

interface Props {
  /** The current SQL source state (mode + raw SQL + builder query + format). */
  value: SqlSourceState;
  /** Called with the next state on any edit. */
  onChange: (state: SqlSourceState) => void;
  /** The dialect to emit. `surreal` for native `store.query`; `standard` for federation. */
  dialect: SqlDialect;
  /** The table/column dropdown source. The HOST loads this (readSchema for local,
   *  discoverTables/describeTable for federation) — the editor stays transport-agnostic. An empty
   *  `tables: []` degrades honestly: the Code half still works (the author types raw SQL). */
  schema: Schema;
  /** Opt-in PRQL support: when true (a standard-dialect host that compiles PRQL at Run — the query
   *  workbench), the Code header shows the SQL|PRQL language toggle bound to `value.lang`. Hosts
   *  that run the raw string verbatim leave it off — offering PRQL there would emit a false promise. */
  allowPrql?: boolean;
}

/** The Builder⇄Code SQL editor. */
export function SqlQueryEditor({ value, onChange, dialect, schema, allowPrql = false }: Props) {
  const switchMode = (mode: typeof value.mode) => {
    if (mode === value.mode) return;
    if (mode === "builder") {
      // Code→Builder: parse the CURRENT raw SQL back into the typed builder query (the three views
      // are projections of one model — the projection runs both ways). Whenever the SQL is
      // expressible in the model the switch is free: the builder shows exactly what the author
      // wrote, and the raw string is left intact. This also covers run-history restore and
      // saved-query load (both land as `{mode:"code", builder:undefined}` and pass through here).
      if (!value.rawSql.trim()) {
        const builder = value.builder ?? emptyQuery();
        onChange({ ...value, mode: "builder", builder, rawSql: emitSql(dialect, builder) });
        return;
      }
      // PRQL is never parseable into the builder model — go straight to the confirm path.
      const parsed = value.lang === "prql" ? null : parseSql(dialect, value.rawSql);
      if (parsed) {
        onChange({ ...value, mode: "builder", builder: parsed });
        return;
      }
      // Not expressible (subquery/CTE/window fn/multi-statement/unparseable): confirm, then keep the
      // raw SQL untouched and start the builder from the salvaged FROM table (or empty).
      const ok =
        typeof window === "undefined" ||
        window.confirm(
          "This SQL uses features the visual builder can't show; switching will keep your SQL but the builder starts empty.",
        );
      if (!ok) return;
      onChange({ ...value, mode: "builder", builder: emptyQuery(salvageFromTable(value.rawSql)) });
    } else {
      // Builder→Code: free — the raw string is already in sync (Builder regenerates it on every edit).
      onChange({ ...value, mode: "code" });
    }
  };

  // Format the raw SQL in place via sql-formatter. The action is GATED to Code mode + standard
  // dialect + SQL lang — Builder regenerates SQL on every edit (formatting would be clobbered),
  // sql-formatter has no SurrealQL grammar (its `sql` fallback corrupts Surreal syntax), and no
  // PRQL grammar either.
  const formatRawSql = useCallback(() => {
    if (value.mode !== "code" || dialect !== "standard" || value.lang === "prql") return;
    const formatted = formatSql(value.rawSql, dialect);
    if (formatted !== value.rawSql) onChange({ ...value, rawSql: formatted });
  }, [value, dialect, onChange]);

  // Cmd/Ctrl+Shift+F — the muscle-memory Format keybinding. Window-level so it works whether or not
  // the CodeMirror textarea has focus. Active only in Code mode + standard dialect (same gate as the
  // button).
  useEffect(() => {
    if (value.mode !== "code" || dialect !== "standard" || value.lang === "prql") return;
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && (e.key === "F" || e.key === "f")) {
        e.preventDefault();
        formatRawSql();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [value.mode, value.lang, dialect, formatRawSql]);

  return (
    <div className="mt-2 flex min-h-0 flex-1 flex-col gap-2" aria-label="sql query editor">
      <SqlQueryHeader
        mode={value.mode}
        format={value.format}
        dialect={dialect}
        onModeChange={switchMode}
        onFormatChange={(format) => onChange({ ...value, format })}
        onFormat={formatRawSql}
        lang={value.lang ?? "sql"}
        onLangChange={allowPrql ? (lang) => onChange({ ...value, lang }) : undefined}
      />

      <div className="flex min-h-0 flex-1 flex-col">
        {value.mode === "builder" ? (
          <VisualEditor
            schema={schema}
            dialect={dialect}
            query={value.builder ?? emptyQuery()}
            onChange={(builder) =>
              // Builder is the source of truth — regenerate the raw SQL on every change.
              onChange({ ...value, builder, rawSql: emitSql(dialect, builder) })
            }
            layout={value.builderLayout}
            onLayoutChange={(builderLayout) => onChange({ ...value, builderLayout })}
          />
        ) : (
          <RawEditor
            rawSql={value.rawSql}
            onChange={(rawSql) => onChange({ ...value, rawSql })}
            schema={schema}
            dialect={dialect}
          />
        )}
      </div>
    </div>
  );
}

export { emptySqlSource };
