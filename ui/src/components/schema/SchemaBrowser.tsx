// A reusable store-schema browser — a collapsible, keyboard-navigable table → column tree over a
// `Schema` (from the shared `@/lib/schema` reader), with a click-to-pick callback (rules-editor-ux
// scope). Built once so any surface that needs to browse local tables/columns reuses it (the rules data
// explorer consumes it now). Pure presentation: it owns only expand/collapse state; the parent decides
// what `onPick(table, column?)` does (insert a snippet, set a query, …). One component per file.

import { useState } from "react";
import { ChevronDown, ChevronRight, Table2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Schema } from "@/lib/schema";

interface SchemaBrowserProps {
  schema: Schema;
  /** Called when a table header (no `column`) or a column is clicked. */
  onPick: (table: string, column?: string) => void;
}

/** A table → column tree with click-to-pick. Tolerates an empty schema (the parent shows the empty/deny
 *  state; this renders nothing for `tables: []`). */
export function SchemaBrowser({ schema, onPick }: SchemaBrowserProps) {
  return (
    <ul aria-label="schema browser" className="grid gap-0.5">
      {schema.tables.map((t) => (
        <SchemaTableRow
          key={t.name}
          name={t.name}
          columns={t.columns.map((c) => c.name)}
          onPick={onPick}
        />
      ))}
    </ul>
  );
}

/** One table: a clickable header that inserts the table and toggles its columns. */
function SchemaTableRow({
  name,
  columns,
  onPick,
}: {
  name: string;
  columns: string[];
  onPick: (table: string, column?: string) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <li>
      <div className="flex items-center gap-0.5">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          aria-label={`toggle table ${name}`}
          aria-expanded={open}
          className="h-6 w-6 shrink-0 text-muted"
          onClick={() => setOpen((v) => !v)}
        >
          {open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          aria-label={`insert table ${name}`}
          className="h-6 flex-1 justify-start gap-1.5 px-1.5 font-mono text-xs text-fg"
          onClick={() => onPick(name)}
        >
          <Table2 size={12} className="text-muted" />
          {name}
        </Button>
      </div>
      {open ? (
        <ul className="ml-6 grid gap-0.5 border-l border-border pl-1.5">
          {columns.length === 0 ? (
            <li className="px-1.5 py-1 text-[11px] text-muted">no columns</li>
          ) : (
            columns.map((c) => (
              <li key={c}>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  aria-label={`insert column ${name}.${c}`}
                  className="h-6 w-full justify-start px-1.5 font-mono text-[11px] text-muted hover:text-fg"
                  onClick={() => onPick(name, c)}
                >
                  {c}
                </Button>
              </li>
            ))
          )}
        </ul>
      ) : null}
    </li>
  );
}
