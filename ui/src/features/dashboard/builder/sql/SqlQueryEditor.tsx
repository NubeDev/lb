// The Grafana-style Builder⇄Code SQL editor (widget-builder Slice C) — ported from Grafana's
// `QueryEditor.tsx`. It switches between the visual `VisualEditor` (Builder) and the raw `RawEditor`
// (Code) by `editorMode`, keeping the two in sync:
//   - Builder edits the typed `SqlBuilderQuery`; every change regenerates the raw SQL via `emitSql`
//     for the editor's `dialect` (Builder is the source of truth in Builder mode).
//   - Code edits the raw string directly (the source of truth in Code mode).
//   - Builder→Code: free (regenerate the string).
//   - Code→Builder: CONFIRM first — hand-edited SQL may not round-trip, so switching back can
//     clobber it (Grafana's behaviour). On confirm we keep the existing builder query (the last
//     visual state).
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
}

/** The Builder⇄Code SQL editor. */
export function SqlQueryEditor({ value, onChange, dialect, schema }: Props) {
  const switchMode = (mode: typeof value.mode) => {
    if (mode === value.mode) return;
    if (mode === "builder") {
      // Code→Builder: confirm, because the typed builder may not represent the hand-edited SQL and we
      // would regenerate from the (possibly stale) builder query — clobbering the raw string.
      const ok =
        typeof window === "undefined" ||
        window.confirm(
          "Switch to Builder? Hand-edited SQL may be replaced by the visual query.",
        );
      if (!ok) return;
      const builder = value.builder ?? emptyQuery();
      onChange({ ...value, mode: "builder", builder, rawSql: emitSql(dialect, builder) });
    } else {
      // Builder→Code: free — the raw string is already in sync (Builder regenerates it on every edit).
      onChange({ ...value, mode: "code" });
    }
  };

  // Format the raw SQL in place via sql-formatter. The action is GATED to Code mode + standard
  // dialect — Builder regenerates SQL on every edit (formatting would be clobbered), and
  // sql-formatter has no SurrealQL grammar (its `sql` fallback corrupts Surreal syntax).
  const formatRawSql = useCallback(() => {
    if (value.mode !== "code" || dialect !== "standard") return;
    const formatted = formatSql(value.rawSql, dialect);
    if (formatted !== value.rawSql) onChange({ ...value, rawSql: formatted });
  }, [value, dialect, onChange]);

  // Cmd/Ctrl+Shift+F — the muscle-memory Format keybinding. Window-level so it works whether or not
  // the CodeMirror textarea has focus. Active only in Code mode + standard dialect (same gate as the
  // button).
  useEffect(() => {
    if (value.mode !== "code" || dialect !== "standard") return;
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && (e.key === "F" || e.key === "f")) {
        e.preventDefault();
        formatRawSql();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [value.mode, dialect, formatRawSql]);

  return (
    <div className="mt-2 grid gap-2" aria-label="sql query editor">
      <SqlQueryHeader
        mode={value.mode}
        format={value.format}
        dialect={dialect}
        onModeChange={switchMode}
        onFormatChange={(format) => onChange({ ...value, format })}
        onFormat={formatRawSql}
      />

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
  );
}

export { emptySqlSource };
