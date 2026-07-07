// The raw SurrealQL CodeMirror editor (widget-builder Slice B) — ported from rubix-cube's
// `components/sql/sql-editor.tsx`, with the `/api/.../sql/generate` AI button DROPPED (re-pointing it
// at an MCP `sql.generate` tool is a named follow-up, out of scope) and the REST/posthog data layer
// removed. It is a pure controlled component: `value`/`onChange` over a SurrealQL string.
//
// This is the **Code** half of the Grafana-style Builder⇄Code SQL source (Slice C, `RawEditor.tsx`
// wraps it). The string it edits is run by `store.query` (Slice A) — parse-allowlisted to a single
// SELECT + bounded + workspace-walled at the host. `@codemirror/lang-sql`'s SQL dialect is close
// enough to SurrealQL for highlighting (a SurrealQL grammar refinement is a named follow-up).
//
// Slice 2 (schema-aware completion): when a `schema` prop is provided the editor builds a
// `schemaConfig(dialect, schema)` and feeds it to `sql(...)` so lang-sql offers table/column/keyword
// completion. Without `schema` the editor stays exactly as before — honest degrade (no completion).
// One responsibility per file (FILE-LAYOUT).

import { sql } from "@codemirror/lang-sql";
import CodeMirror from "@uiw/react-codemirror";
import { useMemo } from "react";

import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";
import type { Schema } from "@/lib/schema";

import { schemaConfig } from "./sqlCompletion";
import { useCodeMirrorTheme } from "./theme";

interface Props {
  /** The current SurrealQL string. */
  value: string;
  /** Called with the new string on every edit. */
  onChange: (value: string) => void;
  /** When false the editor is read-only (e.g. a Builder-generated preview). */
  editable?: boolean;
  placeholder?: string;
  height?: string;
  /** When provided, table/column/keyword completion is enabled (workspace-walled — completion
   *  can only offer what the schema contains). Absent ⇒ today's behaviour (no completion). */
  schema?: Schema;
  /** The SQL grammar dialect for highlighting + completion. Defaults to `standard` (PostgreSQL,
   *  the safe superset for sqlite/postgres/timescale) — the historical behaviour. `surreal` falls
   *  back to StandardSQL (no SurrealQL grammar ships). */
  dialect?: SqlDialect;
}

/** A raw SurrealQL CodeMirror editor (the Code half of the SQL source). */
export function SqlEditor({
  value,
  onChange,
  editable = true,
  placeholder = "SELECT … FROM … (read-only — a single SELECT)",
  height = "120px",
  schema,
  dialect,
}: Props) {
  const cm = useCodeMirrorTheme();
  // Build the lang-sql extension only when schema is provided (absent ⇒ today's bare `sql()`,
  // no completion — honest degrade). useMemo so the extension identity is stable across renders
  // unless the inputs change (avoids CodeMirror re-mounting the language on every keystroke).
  const sqlExtension = useMemo(
    () => (schema ? sql(schemaConfig(dialect ?? "standard", schema)) : sql()),
    [schema, dialect],
  );
  return (
    <div
      className="overflow-hidden rounded-md border border-border bg-bg"
      aria-label="sql editor"
    >
      <CodeMirror
        value={value}
        onChange={onChange}
        editable={editable}
        placeholder={placeholder}
        extensions={[sqlExtension, ...cm.extensions]}
        theme={cm.theme}
        height={height}
        basicSetup={{ lineNumbers: false, foldGutter: false }}
        className="text-xs"
      />
    </div>
  );
}
