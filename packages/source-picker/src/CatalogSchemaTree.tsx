// The local-store schema tree — a collapsible, keyboard-navigable table → column tree, click-to-pick
// (system-catalog scope, moved in from `ui/src/components/schema/SchemaBrowser.tsx`). One tree in the
// package; both the rules panel and any future schema-picker re-point to it. Pure presentation: it
// owns only expand/collapse state; the parent decides what `onSelect` does (insert a snippet, set a
// query, …).
//
// The COLLAPSIBLE primitive is shadcn's file-tree pattern (sidebar-11): Radix Collapsible + a
// `ChevronRight` that rotates 90° on open + an indented sub-tree bordered by a left guide line. The
// package takes `@radix-ui/react-collapsible` + `lucide-react` as peer deps (the same primitives the
// host already uses through shadcn) — the package stays self-themed via `--sp-*` tokens, but the tree
// UX matches the rest of the app.
//
// The click yields a `CatalogEntry` of kind `table` or `column`; the host maps it onto its snippet
// (rule 10 — the package doesn't know what the pick MEANS).

import { ChevronRight, Table2 } from "lucide-react";
import * as CollapsiblePrimitive from "@radix-ui/react-collapsible";

import type { Schema } from "./types";
import type { CatalogEntry } from "./catalog";

export interface CatalogSchemaTreeProps {
  schema: Schema;
  /** Called when a table header (no `column`) or a column row is clicked. */
  onSelect: (entry: CatalogEntry) => void;
}

/** A table → column tree with click-to-pick, using shadcn's file-tree pattern. Tolerates an empty
 *  schema (the parent shows the teaching-empty/deny; this renders nothing for `tables: []`). */
export function CatalogSchemaTree({ schema, onSelect }: CatalogSchemaTreeProps) {
  return (
    <ul aria-label="schema browser" className="sp-catalog-tree">
      {schema.tables.map((t) => (
        <SchemaTableRow key={t.name} name={t.name} columns={t.columns.map((c) => c.name)} onSelect={onSelect} />
      ))}
    </ul>
  );
}

/** One table row: a Collapsible header whose trigger picks the table OR toggles its column list, plus
 *  a nested column sub-tree. The chevron rotates 90° on open (the shadcn sidebar-11 affordance). */
function SchemaTableRow({
  name,
  columns,
  onSelect,
}: {
  name: string;
  columns: string[];
  onSelect: (entry: CatalogEntry) => void;
}) {
  return (
    <li>
      <CollapsiblePrimitive.Root className="group/collapsible sp-catalog-tree-row" defaultOpen={false}>
        <div className="sp-catalog-tree-row-inner">
          <CollapsiblePrimitive.Trigger
            aria-label={`toggle table ${name}`}
            className="sp-catalog-toggle"
          >
            <ChevronRight className="sp-catalog-chevron" />
          </CollapsiblePrimitive.Trigger>
          <button
            type="button"
            aria-label={`insert table ${name}`}
            className="sp-catalog-tree-table"
            onClick={() => onSelect({ kind: "table", id: `table:${name}`, table: name })}
          >
            <Table2 aria-hidden="true" className="sp-catalog-icon" size={12} />
            <span className="sp-catalog-tree-table-name">{name}</span>
          </button>
        </div>
        <CollapsiblePrimitive.Content className="sp-catalog-tree-content">
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
        </CollapsiblePrimitive.Content>
      </CollapsiblePrimitive.Root>
    </li>
  );
}
