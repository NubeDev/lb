// The schema-designer side panel (schema-designer scope) — the properties editor for the selected
// table node. Renders the table name + its columns in a vertical list (name/type/nullable/PK) with
// inline editing, plus FK on-delete policy + delete-table. The canvas mirrors the edits into the
// graph in real time. shadcn-first (Button/Input/Select/Badge). One responsibility, one file.

import { KeyRound, Table2, Trash2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { NEUTRAL_TYPES } from "@/lib/datasources";
import type { EditableTableNodeData } from "./recordFlow";

interface Props {
  /** The selected table's data, or null when nothing is selected. */
  data: EditableTableNodeData | null;
  onRename: (name: string) => void;
  onRenameColumn: (idx: number, name: string) => void;
  onChangeType: (idx: number, type: string) => void;
  onToggleNullable: (idx: number) => void;
  onTogglePk: (idx: number) => void;
  onDeleteTable: () => void;
}

/** The side panel — visible when a table is selected. `null` data renders an empty-state hint. */
export function TableSidePanel({
  data,
  onRename,
  onRenameColumn,
  onChangeType,
  onToggleNullable,
  onTogglePk,
  onDeleteTable,
}: Props) {
  if (!data) {
    return (
      <div className="flex h-full flex-col items-center justify-center bg-bg p-4 text-center text-sm text-muted">
        <Table2 size={20} className="mb-2 opacity-50" />
        <p>Select a table to edit its columns, or drag column→column to create a relationship.</p>
      </div>
    );
  }
  return (
    <div className="flex h-full min-w-0 flex-col bg-bg" data-testid="table-side-panel">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2">
        <Table2 size={13} className="shrink-0 text-accent" />
        <Input
          aria-label="table name"
          value={data.name}
          onChange={(e) => onRename(e.target.value)}
          className="h-6 border-0 bg-transparent px-0 font-mono text-xs font-medium text-accent focus-visible:ring-1"
        />
        <Badge variant="secondary" className="ml-auto font-mono text-[10px]">
          {data.columns.length} col{data.columns.length === 1 ? "" : "s"}
        </Badge>
        <Button
          size="icon"
          variant="ghost"
          className="h-6 w-6 text-muted hover:text-destructive"
          aria-label="delete table"
          onClick={onDeleteTable}
        >
          <Trash2 size={13} />
        </Button>
      </div>

      <div className="flex items-center gap-2 border-b border-border bg-panel/50 px-3 py-1 text-[10px] font-medium uppercase tracking-wide text-muted">
        <span>column</span>
        <span className="ml-auto flex items-center gap-2">
          <KeyRound size={9} /> PK
          <span className="w-12 text-center">type</span>
          <span className="w-6 text-center">null</span>
        </span>
      </div>

      <div className="flex-1 overflow-auto">
        {data.columns.length === 0 && (
          <p className="px-3 py-4 text-xs text-muted">No columns. Use + to add one on the canvas.</p>
        )}
        {data.columns.map((c, i) => (
          <div key={i} className="flex items-center gap-2 border-b border-border/60 px-3 py-1.5">
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={c.pk ? "remove primary key" : "make primary key"}
              aria-pressed={c.pk}
              onClick={() => onTogglePk(i)}
              className={`h-5 w-5 shrink-0 font-mono text-xs ${c.pk ? "text-accent" : "text-muted/40 hover:text-muted"}`}
              title={c.pk ? "primary key" : "toggle PK"}
            >
              {c.pk ? "★" : "☆"}
            </Button>
            <Input
              aria-label={`column ${c.name} name`}
              value={c.name}
              onChange={(e) => onRenameColumn(i, e.target.value)}
              className="h-6 min-w-0 flex-1 border-border bg-bg px-1 font-mono text-[11px] focus-visible:ring-1"
            />
            <Select
              aria-label={`column ${c.name} type`}
              value={c.type}
              onChange={(e) => onChangeType(i, e.target.value)}
              className="h-6 w-20 border-border bg-bg px-1 font-mono text-[10px]"
            >
              {NEUTRAL_TYPES.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </Select>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={c.nullable ? "column nullable" : "column not null"}
              aria-pressed={c.nullable}
              onClick={() => onToggleNullable(i)}
              className={`h-5 w-6 shrink-0 text-[11px] ${c.nullable ? "text-muted" : "text-accent"}`}
              title={c.nullable ? "nullable" : "NOT NULL"}
            >
              {c.nullable ? "?" : "!"}
            </Button>
          </div>
        ))}
      </div>
    </div>
  );
}
