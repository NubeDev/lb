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

  return (
    <div className="mt-2 grid gap-2" aria-label="sql query editor">
      <SqlQueryHeader
        mode={value.mode}
        format={value.format}
        onModeChange={switchMode}
        onFormatChange={(format) => onChange({ ...value, format })}
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
        />
      ) : (
        <RawEditor rawSql={value.rawSql} onChange={(rawSql) => onChange({ ...value, rawSql })} />
      )}
    </div>
  );
}

export { emptySqlSource };
