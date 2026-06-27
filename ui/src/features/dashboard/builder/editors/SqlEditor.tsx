// The raw SurrealQL CodeMirror editor (widget-builder Slice B) — ported from rubix-cube's
// `components/sql/sql-editor.tsx`, with the `/api/.../sql/generate` AI button DROPPED (re-pointing it
// at an MCP `sql.generate` tool is a named follow-up, out of scope) and the REST/posthog data layer
// removed. It is a pure controlled component: `value`/`onChange` over a SurrealQL string.
//
// This is the **Code** half of the Grafana-style Builder⇄Code SQL source (Slice C, `RawEditor.tsx`
// wraps it). The string it edits is run by `store.query` (Slice A) — parse-allowlisted to a single
// SELECT + bounded + workspace-walled at the host. `@codemirror/lang-sql`'s SQL dialect is close
// enough to SurrealQL for highlighting (a SurrealQL grammar refinement is a named follow-up).
// One responsibility per file (FILE-LAYOUT).

import { sql } from "@codemirror/lang-sql";
import CodeMirror from "@uiw/react-codemirror";

import { baseExtensions } from "./theme";

interface Props {
  /** The current SurrealQL string. */
  value: string;
  /** Called with the new string on every edit. */
  onChange: (value: string) => void;
  /** When false the editor is read-only (e.g. a Builder-generated preview). */
  editable?: boolean;
  placeholder?: string;
  height?: string;
}

/** A raw SurrealQL CodeMirror editor (the Code half of the SQL source). */
export function SqlEditor({
  value,
  onChange,
  editable = true,
  placeholder = "SELECT … FROM … (read-only — a single SELECT)",
  height = "120px",
}: Props) {
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
        extensions={[sql(), ...baseExtensions]}
        height={height}
        basicSetup={{ lineNumbers: false, foldGutter: false }}
        className="text-xs"
      />
    </div>
  );
}
