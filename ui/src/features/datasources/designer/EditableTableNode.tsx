// The editable schema-table node (schema-designer scope) — adapted from tabularis's read-only
// `SchemaTableNode` (Apache-2.0), with inline editing added. Each column row is an editable name +
// type `<select>` + nullable toggle + PK star; each row carries left `target` + right `source`
// Handles (id = column name) so dragging column→column creates an FK edge (declared, not inferred).
//
// The node reads its update callback from a React Context (`SchemaDesignerNodeContext`) that the
// canvas provides — xyflow v12's `NodeProps` doesn't accept custom props beyond `data`, and
// stamping callbacks into `data` fights the record-as-source-of-truth. Context keeps the node a
// pure renderer of its data + a thin editor. One responsibility, one file (FILE-LAYOUT).
//
// **Provenance:** the handle-per-column + table-header + row layout skeleton is from tabularis
// (Apache-2.0). The inline `<Input>`/`<Select>` editing, add/remove-column, PK toggle, and the
// "add column" footer are original. ChartDB is AGPL-3.0 — UX reference only, no code copied.

import { createContext, memo, useContext } from "react";
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { Plus, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { NEUTRAL_TYPES } from "@/lib/datasources";
import type { EditableTableNodeData } from "./recordFlow";

/** The context the canvas provides so a node can report an edit without a callback in `data`.
 *  `null` means the node is rendering outside the designer (e.g. a storybook) → edits are no-ops. */
export const SchemaDesignerNodeContext = createContext<
  ((id: string, next: EditableTableNodeData) => void) | null
>(null);

/** The node-type key the canvas registers under (`nodeTypes = { editableTable: EditableTableNode }`). */
export const SCHEMA_TABLE_NODE_TYPE = "editableTable" as const;

function EditableTableNodeImpl({ data, id, selected }: NodeProps<Node<EditableTableNodeData>>) {
  const update = useContext(SchemaDesignerNodeContext);
  const { name, columns } = data;

  const commit = (mutate: (d: EditableTableNodeData) => EditableTableNodeData) => {
    if (update) update(id, mutate(data));
  };
  const renameTable = (newName: string) =>
    commit((d) => ({ ...d, name: newName }));
  const renameColumn = (idx: number, colName: string) =>
    commit((d) => ({
      ...d,
      columns: d.columns.map((c, i) => (i === idx ? { ...c, name: colName } : c)),
    }));
  const changeType = (idx: number, type: string) =>
    commit((d) => ({
      ...d,
      columns: d.columns.map((c, i) => (i === idx ? { ...c, type } : c)),
    }));
  const toggleNullable = (idx: number) =>
    commit((d) => ({
      ...d,
      columns: d.columns.map((c, i) => (i === idx ? { ...c, nullable: !c.nullable } : c)),
    }));
  const togglePk = (idx: number) =>
    commit((d) => ({
      ...d,
      columns: d.columns.map((c, i) => (i === idx ? { ...c, pk: !c.pk } : c)),
    }));
  const addColumn = () =>
    commit((d) => {
      const n = d.columns.length + 1;
      return {
        ...d,
        columns: [...d.columns, { name: `col_${n}`, type: "text", nullable: true, pk: false }],
      };
    });
  const removeColumn = (idx: number) =>
    commit((d) => ({ ...d, columns: d.columns.filter((_, i) => i !== idx) }));

  return (
    <div
      className={`w-72 overflow-hidden rounded-md border bg-panel shadow-sm ${
        selected ? "border-accent/70 ring-2 ring-accent/20" : "border-border"
      }`}
      data-testid="schema-table-node"
    >
      <div className="flex items-center gap-2 border-b border-border bg-bg/50 px-3 py-2">
        <Input
          aria-label="table name"
          value={name}
          onChange={(e) => renameTable(e.target.value)}
          className="h-6 border-0 bg-transparent px-0 font-mono text-xs font-medium text-accent focus-visible:ring-0"
        />
        <span className="ml-auto shrink-0 rounded-md border border-border bg-bg px-1.5 py-0.5 text-[10px] text-muted">
          {columns.length} col{columns.length === 1 ? "" : "s"}
        </span>
      </div>

      <div className="max-h-80 overflow-auto">
        {columns.length === 0 && (
          <div className="px-3 py-2 text-[11px] text-muted">No columns yet.</div>
        )}
        {columns.map((c, i) => (
          <div
            key={i}
            className="relative flex items-center gap-1.5 border-b border-border/60 px-3 py-1 last:border-b-0"
          >
            <Handle
              id={c.name}
              type="target"
              position={Position.Left}
              className="h-1.5 w-1.5 border-0 bg-muted/60"
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={c.pk ? "remove primary key" : "make primary key"}
              aria-pressed={c.pk}
              onClick={() => togglePk(i)}
              className={`h-4 w-4 shrink-0 font-mono text-[11px] ${
                c.pk ? "text-accent" : "text-muted/40 hover:text-muted"
              }`}
              title={c.pk ? "primary key" : "toggle PK"}
            >
              {c.pk ? "★" : "☆"}
            </Button>
            <Input
              aria-label={`column ${c.name} name`}
              value={c.name}
              onChange={(e) => renameColumn(i, e.target.value)}
              className="h-5 min-w-0 flex-1 border-0 bg-transparent px-0 font-mono text-[11px] text-fg focus-visible:ring-1"
            />
            <Select
              aria-label={`column ${c.name} type`}
              value={c.type}
              onChange={(e) => changeType(i, e.target.value)}
              className="h-5 w-20 border-border bg-bg px-1 font-mono text-[10px] text-muted"
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
              onClick={() => toggleNullable(i)}
              className={`h-4 w-4 shrink-0 text-[10px] ${c.nullable ? "text-muted" : "text-accent"}`}
              title={c.nullable ? "nullable" : "NOT NULL"}
            >
              {c.nullable ? "?" : "!"}
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              aria-label={`remove column ${c.name}`}
              onClick={() => removeColumn(i)}
              className="h-4 w-4 shrink-0 text-muted/40 hover:text-destructive"
            >
              <Trash2 size={11} />
            </Button>
            <Handle
              id={c.name}
              type="source"
              position={Position.Right}
              className="h-1.5 w-1.5 border-0 bg-muted/60"
            />
          </div>
        ))}
      </div>

      <Button
        type="button"
        variant="ghost"
        onClick={addColumn}
        className="flex w-full items-center justify-center gap-1.5 rounded-none border-t border-border bg-bg/30 px-3 py-1.5 text-[11px] text-muted hover:bg-bg/60 hover:text-fg"
        aria-label="add column"
      >
        <Plus size={11} /> add column
      </Button>
    </div>
  );
}

void Button;

export const EditableTableNode = memo(EditableTableNodeImpl);
EditableTableNode.displayName = "EditableTableNode";
