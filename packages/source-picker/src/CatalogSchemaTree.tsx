// The local-store schema tree — a collapsible, keyboard-navigable table → column tree, click-to-pick
// (system-catalog scope, moved in from `ui/src/components/schema/SchemaBrowser.tsx`). One tree in the
// package; both the rules panel and any future schema-picker re-point to it. Pure presentation: it
// owns only expand/collapse state; the parent decides what `onSelect` does (insert a snippet, set a
// query, …).
//
// The click yields a `CatalogEntry` of kind `table` or `column`; the host maps it onto its snippet
// (rule 10 — the package doesn't know what the pick MEANS). Self-themed via `--sp-*` tokens.

import { useState } from "react";

import type { Schema } from "./types";
import type { CatalogEntry } from "./catalog";

export interface CatalogSchemaTreeProps {
  schema: Schema;
  /** Called when a table header (no `column`) or a column row is clicked. */
  onSelect: (entry: CatalogEntry) => void;
}

/** A table → column tree with click-to-pick. Tolerates an empty schema (the parent shows the
 *  teaching-empty/deny; this renders nothing for `tables: []`). */
export function CatalogSchemaTree({ schema, onSelect }: CatalogSchemaTreeProps) {
  return (
    <ul aria-label="schema browser" className="sp-catalog-tree">
      {schema.tables.map((t) => (
        <SchemaTableRow key={t.name} name={t.name} columns={t.columns.map((c) => c.name)} onSelect={onSelect} />
      ))}
    </ul>
  );
}

/** One table: a clickable header that yields a `table` entry + toggles its column list. */
function SchemaTableRow({
  name,
  columns,
  onSelect,
}: {
  name: string;
  columns: string[];
  onSelect: (entry: CatalogEntry) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <li>
      <div className="sp-catalog-tree-row">
        <button
          type="button"
          aria-label={`toggle table ${name}`}
          aria-expanded={open}
          className="sp-catalog-toggle"
          onClick={() => setOpen((v) => !v)}
        >
          {open ? "▾" : "▸"}
        </button>
        <button
          type="button"
          aria-label={`insert table ${name}`}
          className="sp-catalog-tree-table"
          onClick={() => onSelect({ kind: "table", id: `table:${name}`, table: name })}
        >
          <span aria-hidden="true" className="sp-catalog-icon">
            ▦
          </span>
          {name}
        </button>
      </div>
      {open ? (
        <ul className="sp-catalog-tree-columns">
          {columns.length === 0 ? (
            <li className="sp-catalog-tree-no-columns">no columns</li>
          ) : (
            columns.map((c) => (
              <li key={c}>
                <button
                  type="button"
                  aria-label={`insert column ${name}.${c}`}
                  className="sp-catalog-tree-column"
                  onClick={() =>
                    onSelect({ kind: "column", id: `column:${name}.${c}`, table: name, column: c })
                  }
                >
                  {c}
                </button>
              </li>
            ))
          )}
        </ul>
      ) : null}
    </li>
  );
}
