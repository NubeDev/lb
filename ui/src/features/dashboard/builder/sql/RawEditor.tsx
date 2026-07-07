// The raw-SQL half of the Builder⇄Code SQL source (widget-builder Slice C) — ported from Grafana's
// `query-editor-raw/RawEditor.tsx` (with `QueryEditorRaw.tsx` folded in). It wraps Slice B's
// `SqlEditor.tsx` (the CodeMirror SurrealQL editor) and writes the hand-edited string straight back
// into the source. The string is run by `store.query`, still parse-allowlisted to a single SELECT +
// bounded + walled at the host — Code mode does not relax the boundary.
//
// Slice 2: passes the workspace `schema` + `dialect` straight through so Code mode gets the SAME
// schema the Builder dropdowns use (one load, both halves).

import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";
import type { Schema } from "@/lib/schema";

import { SqlEditor } from "../editors/SqlEditor";

interface Props {
  /** The raw SurrealQL string. */
  rawSql: string;
  /** Called with the edited string. */
  onChange: (sql: string) => void;
  /** The workspace schema — feeds table/column completion in Code mode. */
  schema: Schema;
  /** The SQL dialect — picks the lang-sql grammar + completion shape. */
  dialect: SqlDialect;
}

/** The Code-mode raw SurrealQL editor (wraps the CodeMirror `SqlEditor`). */
export function RawEditor({ rawSql, onChange, schema, dialect }: Props) {
  return (
    <div className="grid gap-1">
      <SqlEditor
        value={rawSql}
        onChange={onChange}
        height="140px"
        schema={schema}
        dialect={dialect}
      />
      <p className="text-[10px] text-muted">
        Read-only: a single <span className="font-mono">SELECT</span>. A write/multi/namespace statement
        is refused at the host (parse-allowlisted), bounded to 10k rows / 5s, and runs only in your
        workspace.
      </p>
    </div>
  );
}
