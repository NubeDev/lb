// The schema-table node for the datasource ERD (datasources-ux ERD scope) — adapted from React Flow's
// "database schema node" pattern (https://reactflow.dev/ui/components/database-schema-node), rewritten
// against THIS repo's design tokens + shadcn-first conventions (no shadcn-registry dependency, no new
// npm dep). One node per table: a header (table name + column count) over a scrollable list of columns
// (name + type), each row carrying a left `target` + right `source` Handle (id = column name) so
// inferred FK edges attach column-to-column. Pure render of {@link SchemaTableNodeData}; the projection
// + layout live next to it. One responsibility, one file (FILE-LAYOUT).

import { memo } from "react";
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { Table2 } from "lucide-react";

import type { SchemaTableNodeData } from "./schemaToFlow";

function SchemaTableNodeImpl({ data, selected }: NodeProps<Node<SchemaTableNodeData>>) {
  const { name, columns } = data;
  return (
    <div
      className={`w-64 overflow-hidden rounded-md border bg-panel shadow-sm transition-colors ${
        selected ? "border-accent/70 ring-2 ring-accent/20" : "border-border"
      }`}
    >
      <div className="flex items-center gap-2 border-b border-border bg-bg/50 px-3 py-2">
        <Table2 size={13} className="shrink-0 text-accent" />
        <span className="min-w-0 truncate font-mono text-xs font-medium text-accent">{name}</span>
        <span className="ml-auto shrink-0 rounded-md border border-border bg-bg px-1.5 py-0.5 text-[10px] text-muted">
          {columns.length} col{columns.length === 1 ? "" : "s"}
        </span>
      </div>

      <div className="max-h-72 overflow-auto">
        {columns.length === 0 && (
          <div className="px-3 py-2 text-[11px] text-muted">No columns discovered.</div>
        )}
        {columns.map((c) => (
          <div
            key={c.name}
            className="relative flex items-center gap-2 border-b border-border/60 px-3 py-1 last:border-b-0"
          >
            <Handle
              id={c.name}
              type="target"
              position={Position.Left}
              className="h-1.5 w-1.5 border-0 bg-muted/60"
            />
            <span className="min-w-0 truncate font-mono text-[11px] text-fg">{c.name}</span>
            <span className="ml-auto shrink-0 rounded-md border border-border bg-bg px-1.5 py-0.5 font-mono text-[10px] text-muted">
              {c.dataType}
            </span>
            {c.nullable && (
              <span className="shrink-0 text-[10px] text-muted/70" title="nullable">
                ?
              </span>
            )}
            <Handle
              id={c.name}
              type="source"
              position={Position.Right}
              className="h-1.5 w-1.5 border-0 bg-muted/60"
            />
          </div>
        ))}
      </div>
    </div>
  );
}

export const SchemaTableNode = memo(SchemaTableNodeImpl);
SchemaTableNode.displayName = "SchemaTableNode";
