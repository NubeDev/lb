// Adapted from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0. Interaction design preserved;
// the data layer is rewired onto our typed SqlBuilderQuery (model-as-truth, not nodes-as-truth).
//
// The per-row sub-components for the canvas `QuerySettingsPanel` (visual-canvas-builder slice): one
// filter row (WHERE/HAVING with AND/OR + isAggregate + operator incl. LIKE/IS NULL/IS NOT NULL), one
// groupBy row, one orderBy row. Each calls back with a typed edit; the panel composes them. Extracted
// into its own file so the panel file stays under the 400-line ceiling (FILE-LAYOUT §1).

import { X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import type {
  SqlAggregation,
  SqlFilter,
  SqlLogical,
  SqlOperator,
  SqlOrderBy,
} from "@/lib/panel-kit/sql/query";

/** Shared field styling (mirrors the canvas host's tokens). */
export const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2 text-[11px] text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

/** Operators including the visual-canvas-builder additions (LIKE / IS NULL / IS NOT NULL). */
export const OPERATORS: SqlOperator[] = ["=", "!=", ">", ">=", "<", "<=", "LIKE", "IS NULL", "IS NOT NULL"];

/** Aggregations selectable on a HAVING row (excludes the empty "(none)" entry). */
export const AGGREGATIONS: (SqlAggregation | "")[] = ["", "count", "count_distinct", "sum", "avg", "min", "max"];

/** The value-less operators (no value input renders for these). */
export const VALUELESS: SqlOperator[] = ["IS NULL", "IS NOT NULL"];

/** A qualified column reference (table.column) — the row dropdowns' option shape. */
export interface QualifiedColumn {
  table: string;
  column: string;
}

/** Render `{table, column}` as the option value `table.column` (or `.column` if no table). */
export function columnKey(table: string | undefined, column: string): string {
  return table ? `${table}.${column}` : `.${column}`;
}

/** Split an option value `table.column` back into `{table, column}` (table may be empty). */
export function splitColumnKey(key: string): QualifiedColumn {
  const idx = key.indexOf(".");
  if (idx < 0) return { table: "", column: key };
  return { table: key.slice(0, idx), column: key.slice(idx + 1) };
}

/** One WHERE/HAVING filter row — column, optional logical prefix, isAggregate toggle, operator+value. */
export function FilterRow({
  filter,
  columns,
  showLogical,
  onChange,
  onRemove,
}: {
  filter: SqlFilter;
  columns: QualifiedColumn[];
  showLogical: boolean;
  onChange: (f: SqlFilter) => void;
  onRemove: () => void;
}) {
  const valueless = VALUELESS.includes(filter.operator);
  return (
    <div className="grid gap-1 rounded-md border border-border/60 bg-bg/40 p-2">
      {showLogical && (
        <div className="flex gap-1">
          {(["AND", "OR"] as SqlLogical[]).map((l) => (
            <Button
              key={l}
              type="button"
              variant="ghost"
              onClick={() => onChange({ ...filter, logical: l })}
              className={`flex-1 rounded-md px-2 py-0.5 text-[10px] font-medium transition-colors ${
                (filter.logical ?? "AND") === l
                  ? "bg-accent text-bg hover:bg-accent hover:text-bg"
                  : "bg-bg text-muted hover:text-fg"
              }`}
            >
              {l}
            </Button>
          ))}
        </div>
      )}
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
      <select
        aria-label="sql filter column"
        className={FIELD}
        value={columnKey(filter.table, filter.column)}
        onChange={(e) => {
          const { table, column } = splitColumnKey(e.target.value);
          onChange({ ...filter, column, table });
        }}
      >
        {columns.map((c) => (
          <option key={`${c.table}.${c.column}`} value={`${c.table}.${c.column}`}>
            {c.table}.{c.column}
          </option>
        ))}
      </select>
      <label className="flex items-center gap-1.5 text-[10px] text-muted">
        <Checkbox
          checked={!!filter.isAggregate}
          onChange={(e) => onChange({ ...filter, isAggregate: e.target.checked })}
          className="h-3 w-3 rounded-md border-border bg-bg text-accent focus:ring-0"
        />
        HAVING (aggregate)
      </label>
      {filter.isAggregate && (
        /* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */
        <select
          aria-label="sql filter aggregation"
          className={FIELD}
          value={filter.aggregation ?? ""}
          onChange={(e) =>
            onChange({
              ...filter,
              aggregation: (e.target.value || undefined) as SqlAggregation | undefined,
            })
          }
        >
          {AGGREGATIONS.filter((a) => a !== "").map((a) => (
            <option key={a} value={a}>
              {a}
            </option>
          ))}
        </select>
      )}
      <div className="flex items-center gap-1">
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
        <select
          aria-label="sql filter operator"
          className={`${FIELD} w-20`}
          value={filter.operator}
          onChange={(e) => onChange({ ...filter, operator: e.target.value as SqlOperator })}
        >
          {OPERATORS.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </select>
        {!valueless && (
          <Input
            aria-label="sql filter value"
            className={`${FIELD} flex-1`}
            value={String(filter.value ?? "")}
            onChange={(e) => onChange({ ...filter, value: e.target.value })}
          />
        )}
        <RowIconButton label="remove filter" onClick={onRemove}>
          <X size={12} />
        </RowIconButton>
      </div>
    </div>
  );
}

/** One GROUP BY row — qualified column dropdown + remove. */
export function GroupByRow({
  entry,
  columns,
  onChange,
  onRemove,
}: {
  entry: string | QualifiedColumn;
  columns: QualifiedColumn[];
  onChange: (e: string | QualifiedColumn) => void;
  onRemove: () => void;
}) {
  const key = typeof entry === "string" ? entry : `${entry.table}.${entry.column}`;
  return (
    <div className="flex items-center gap-1">
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
      <select
        aria-label="sql group by"
        className={`${FIELD} flex-1`}
        value={key.includes(".") ? key : `.${key}`}
        onChange={(e) => {
          const { table, column } = splitColumnKey(e.target.value);
          onChange(table ? { table, column } : column);
        }}
      >
        {columns.map((c) => (
          <option key={`${c.table}.${c.column}`} value={`${c.table}.${c.column}`}>
            {c.table}.{c.column}
          </option>
        ))}
      </select>
      <RowIconButton label="remove group by" onClick={onRemove}>
        <X size={12} />
      </RowIconButton>
    </div>
  );
}

/** One ORDER BY row — qualified column + direction + remove. */
export function OrderByRow({
  order,
  columns,
  onChange,
  onRemove,
}: {
  order: SqlOrderBy;
  columns: QualifiedColumn[];
  onChange: (o: SqlOrderBy) => void;
  onRemove: () => void;
}) {
  return (
    <div className="flex items-center gap-1">
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
      <select
        aria-label="sql order column"
        className={`${FIELD} flex-1`}
        value={columnKey(order.table, order.column)}
        onChange={(e) => {
          const { table, column } = splitColumnKey(e.target.value);
          onChange({ ...order, column, table });
        }}
      >
        {columns.map((c) => (
          <option key={`${c.table}.${c.column}`} value={`${c.table}.${c.column}`}>
            {c.table}.{c.column}
          </option>
        ))}
      </select>
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
      <select
        aria-label="sql order direction"
        className={`${FIELD} w-16`}
        value={order.direction}
        onChange={(e) => onChange({ ...order, direction: e.target.value as "asc" | "desc" })}
      >
        <option value="asc">asc</option>
        <option value="desc">desc</option>
      </select>
      <RowIconButton label="remove order" onClick={onRemove}>
        <X size={12} />
      </RowIconButton>
    </div>
  );
}

/** A small ghost-icon remove button shared by the rows. */
export function RowIconButton({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      aria-label={label}
      onClick={onClick}
      className="h-7 w-7 shrink-0 text-muted"
    >
      {children}
    </Button>
  );
}
