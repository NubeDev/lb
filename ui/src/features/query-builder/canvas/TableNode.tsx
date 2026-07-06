// Adapted from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0. Interaction design preserved;
// the data layer is rewired onto our typed SqlBuilderQuery (model-as-truth, not nodes-as-truth).
//
// One table node on the canvas — a box with checkable columns + a per-column options popover
// (visual-canvas-builder slice). Checkbox toggles column selection; the gear reveals an inline
// aggregation dropdown (incl. `count_distinct`), an alias input, and a position (order) input. The
// right/left handles carry the column name as their id, so a connect drag resolves to a SqlJoin.
// Handles reveal on hover. Each edit fires a callback the QueryCanvas host maps to a typed query edit.

import { memo, useState } from "react";
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import type { SqlAggregation } from "@/lib/panel-kit/sql/query";
import type { CanvasNodeData } from "./canvasModel";

/** The per-column runtime state — derived from `query.columns` by the QueryCanvas host. */
export interface ColumnState {
  selected: boolean;
  aggregation?: SqlAggregation;
  alias?: string;
  order?: number;
}

/** The full `data` payload QueryCanvas spreads onto each node (catalog + per-column state + callbacks). */
export interface TableNodeData extends CanvasNodeData {
  /** Per-column state, keyed by column name (selected flag + aggregation/alias/order). */
  columnStates: Record<string, ColumnState>;
  /** True when the dialect supports joins — controls whether source/target handles render. */
  isJoinable: boolean;
  onColumnCheck: (column: string, checked: boolean) => void;
  onColumnAggregation: (column: string, aggregation: SqlAggregation | undefined) => void;
  onColumnAlias: (column: string, alias: string | undefined) => void;
  onColumnOrder: (column: string, order: number | undefined) => void;
  onDelete?: () => void;
}

/** A typed React-Flow node carrying `TableNodeData`. */
export type TableFlowNode = Node<TableNodeData, "table">;

const AGGREGATIONS: (SqlAggregation | "")[] = [
  "",
  "count",
  "count_distinct",
  "sum",
  "avg",
  "min",
  "max",
];

const COL_INPUT =
  "w-full rounded-md border border-border bg-bg px-1.5 py-1 text-[10px] text-fg focus-visible:border-accent focus-visible:outline-none";

/** A table node — header with the table name + delete, body with checkable columns + an inline
 *  options drawer for aggregation/alias/order. Right/left column handles carry the column id. */
export const TableNode = memo(function TableNode({ data }: NodeProps<TableFlowNode>) {
  const [expandedColumn, setExpandedColumn] = useState<string | null>(null);
  const d = data;

  return (
    <div
      className={cn(
        "min-w-[200px] overflow-hidden rounded-md border bg-panel shadow-lg",
        d.pending ? "border-dashed border-warning/60" : "border-border",
      )}
    >
      <div className="relative flex items-center gap-1.5 border-b border-border bg-bg px-3 py-2 text-xs font-semibold text-fg">
        <span className={cn("h-2 w-2 rounded-full", d.pending ? "bg-warning" : "bg-accent")} />
        {d.table}
        {d.pending && (
          <span
            className="rounded-md bg-warning/10 px-1.5 py-0.5 text-[9px] font-medium text-warning"
            title="Drag a column dot to a column on another table to join it into the query"
          >
            not joined
          </span>
        )}
        {d.onDelete && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            onClick={d.onDelete}
            className="absolute right-1.5 top-1.5 h-6 w-6 text-muted hover:bg-bg hover:text-fg"
            title="Remove table"
            aria-label={`remove table ${d.table}`}
          >
            <X size={12} />
          </Button>
        )}
      </div>
      <div className="flex max-h-[300px] flex-col gap-0.5 overflow-y-auto p-1.5">
        {d.columns.length === 0 && (
          <div className="px-2 py-1.5 text-[11px] italic text-muted" aria-label={`columns loading ${d.table}`}>
            loading columns…
          </div>
        )}
        {d.columns.map((col) => {
          const st = d.columnStates[col.name] ?? { selected: false };
          const isExpanded = expandedColumn === col.name;
          return (
            <div key={col.name} className="group relative">
              <div className="flex items-center justify-between px-2 py-0.5 text-[11px] text-muted hover:text-fg">
                <div className="flex flex-1 select-none items-center gap-1.5">
                  <Checkbox
                    className="h-3 w-3 rounded-md border-border bg-bg text-accent focus:ring-0"
                    checked={st.selected}
                    onChange={(e) => d.onColumnCheck(col.name, e.target.checked)}
                    onClick={(e) => e.stopPropagation()}
                    aria-label={`select column ${col.name}`}
                  />
                  <span
                    className="cursor-pointer truncate hover:text-accent"
                    onClick={() => setExpandedColumn(isExpanded ? null : col.name)}
                  >
                    {col.name}
                  </span>
                  {st.aggregation && (
                    <span className="rounded-md bg-accent/10 px-1 font-mono text-[9px] text-accent">
                      {st.aggregation}
                    </span>
                  )}
                  {st.alias && (
                    <span className="rounded-md bg-accent/10 px-1 font-mono text-[9px] text-fg">
                      as {st.alias}
                    </span>
                  )}
                  <span className="ml-auto font-mono text-[9px] text-muted">{col.type}</span>
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  onClick={() => setExpandedColumn(isExpanded ? null : col.name)}
                  className={cn(
                    "ml-1 h-5 w-5 rounded-md p-0.5 text-muted opacity-0 transition-opacity hover:bg-bg hover:text-accent group-hover:opacity-100",
                    isExpanded && "!bg-accent/20 !text-accent !opacity-100",
                  )}
                  title={isExpanded ? "Close options" : "Column options"}
                  aria-label={`column options ${col.name}`}
                >
                  <span className="text-[10px]">{isExpanded ? "×" : "⚙"}</span>
                </Button>
                {d.isJoinable && (
                  <>
                    {/* Always faintly visible (the join gesture must be discoverable), full on
                        row hover. Opacity only — the 10px hit target is always live. */}
                    <Handle
                      type="source"
                      position={Position.Right}
                      id={col.name}
                      className="!right-[-5px] !h-2.5 !w-2.5 !border !border-border !bg-accent !opacity-40 transition-opacity group-hover:!opacity-100"
                    />
                    <Handle
                      type="target"
                      position={Position.Left}
                      id={col.name}
                      className="!left-[-5px] !h-2.5 !w-2.5 !border !border-border !bg-accent !opacity-40 transition-opacity group-hover:!opacity-100"
                    />
                  </>
                )}
              </div>
              {isExpanded && (
                <div className="relative z-50 m-1.5 mt-1 ml-5 space-y-1.5 rounded-md border border-border bg-bg p-2 shadow">
                  <div className="mb-1 text-[10px] font-semibold text-muted">AGGREGATION</div>
                  {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
                  <select
                    aria-label={`aggregation ${col.name}`}
                    className={COL_INPUT}
                    value={st.aggregation ?? ""}
                    onChange={(e) => {
                      const v = e.target.value as SqlAggregation | "";
                      d.onColumnAggregation(col.name, v === "" ? undefined : v);
                    }}
                  >
                    {AGGREGATIONS.map((a) => (
                      <option key={a || "none"} value={a}>
                        {a || "(none)"}
                      </option>
                    ))}
                  </select>
                  <div className="mb-1 mt-2 text-[10px] font-semibold text-muted">ALIAS</div>
                  <Input
                    type="text"
                    aria-label={`alias ${col.name}`}
                    className={COL_INPUT}
                    placeholder="result column name"
                    value={st.alias ?? ""}
                    onChange={(e) => d.onColumnAlias(col.name, e.target.value || undefined)}
                  />
                  <div className="mb-1 mt-2 text-[10px] font-semibold text-muted">POSITION</div>
                  <Input
                    type="number"
                    min={1}
                    aria-label={`order ${col.name}`}
                    className={COL_INPUT}
                    placeholder="select order"
                    value={st.order ?? ""}
                    onChange={(e) =>
                      d.onColumnOrder(col.name, e.target.value ? Number(e.target.value) : undefined)
                    }
                  />
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});
