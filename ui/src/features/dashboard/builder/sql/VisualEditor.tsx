// The visual query builder (widget-builder Slice C) — ported from Grafana's
// `visual-query-builder/VisualEditor.tsx`, rendered with our own primitives (no `@grafana/ui`). The
// rows a non-SQL user fills: Table → Column/Aggregation → Filter (where) → Group by → Order by →
// Limit, with a live SurrealQL preview. The Table/Column dropdowns are populated by `store.schema`
// (Slice A). Editing the typed `SqlBuilderQuery` regenerates the SurrealQL string (via `toSurrealQL`)
// the parent keeps in sync. Builder mode can ONLY generate a SELECT.

import { Plus, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Schema } from "@/lib/schema";
import type {
  SqlAggregation,
  SqlBuilderQuery,
  SqlFilter,
  SqlOperator,
} from "./query";
import { toSurrealQL } from "./toSurrealQL";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

const AGGREGATIONS: (SqlAggregation | "")[] = ["", "count", "sum", "avg", "min", "max"];
const OPERATORS: SqlOperator[] = ["=", "!=", ">", ">=", "<", "<="];

interface Props {
  schema: Schema;
  query: SqlBuilderQuery;
  onChange: (query: SqlBuilderQuery) => void;
}

/** The visual query builder rows + live preview. */
export function VisualEditor({ schema, query, onChange }: Props) {
  const tableNames = schema.tables.map((t) => t.name);
  const columns =
    schema.tables.find((t) => t.name === query.table)?.columns.map((c) => c.name) ?? [];

  const setTable = (table: string) =>
    // A new table invalidates column-bound clauses — reset them honestly rather than carry stale ones.
    onChange({ ...query, table, columns: [], filters: [], groupBy: [], orderBy: undefined });

  const addColumn = () =>
    onChange({ ...query, columns: [...query.columns, { name: columns[0] ?? "*" }] });
  const addFilter = () =>
    onChange({
      ...query,
      filters: [...query.filters, { column: columns[0] ?? "", operator: "=", value: "" }],
    });

  return (
    <div className="grid gap-2" aria-label="sql visual builder">
      {/* Table */}
      <Row label="Table">
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
        <select aria-label="sql table" className={FIELD} value={query.table} onChange={(e) => setTable(e.target.value)}>
          <option value="">— pick a table —</option>
          {tableNames.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
      </Row>

      {/* Columns / aggregations */}
      <Row label="Columns">
        <div className="grid gap-1">
          {query.columns.map((col, i) => (
            <div key={i} className="flex items-center gap-1">
              {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
              <select
                aria-label={`sql column ${i}`}
                className={FIELD}
                value={col.name}
                onChange={(e) => {
                  const next = [...query.columns];
                  next[i] = { ...col, name: e.target.value };
                  onChange({ ...query, columns: next });
                }}
              >
                <option value="*">*</option>
                {columns.map((c) => (
                  <option key={c} value={c}>
                    {c}
                  </option>
                ))}
              </select>
              {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
              <select
                aria-label={`sql aggregation ${i}`}
                className={FIELD}
                value={col.aggregation ?? ""}
                onChange={(e) => {
                  const next = [...query.columns];
                  const agg = e.target.value as SqlAggregation | "";
                  next[i] = { ...col, aggregation: agg === "" ? undefined : agg };
                  onChange({ ...query, columns: next });
                }}
              >
                {AGGREGATIONS.map((a) => (
                  <option key={a || "none"} value={a}>
                    {a || "(none)"}
                  </option>
                ))}
              </select>
              <IconButton label={`remove column ${i}`} onClick={() => onChange({ ...query, columns: query.columns.filter((_, j) => j !== i) })}>
                <X size={12} />
              </IconButton>
            </div>
          ))}
          <AddButton label="add column" onClick={addColumn} />
        </div>
      </Row>

      {/* Filters (WHERE) */}
      <Row label="Filter">
        <div className="grid gap-1">
          {query.filters.map((f, i) => (
            <FilterRow
              key={i}
              filter={f}
              columns={columns}
              onChange={(nf) => {
                const next = [...query.filters];
                next[i] = nf;
                onChange({ ...query, filters: next });
              }}
              onRemove={() => onChange({ ...query, filters: query.filters.filter((_, j) => j !== i) })}
            />
          ))}
          <AddButton label="add filter" onClick={addFilter} />
        </div>
      </Row>

      {/* Group by */}
      <Row label="Group by">
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive (multi via comma) */}
        <select
          aria-label="sql group by"
          multiple
          className={`${FIELD} h-16`}
          value={query.groupBy}
          onChange={(e) =>
            onChange({
              ...query,
              groupBy: Array.from(e.target.selectedOptions).map((o) => o.value),
            })
          }
        >
          {columns.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </select>
      </Row>

      {/* Order by + Limit */}
      <Row label="Order by">
        <div className="flex items-center gap-1">
          {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
          <select
            aria-label="sql order column"
            className={FIELD}
            value={query.orderBy?.column ?? ""}
            onChange={(e) =>
              onChange({
                ...query,
                orderBy: e.target.value
                  ? { column: e.target.value, direction: query.orderBy?.direction ?? "asc" }
                  : undefined,
              })
            }
          >
            <option value="">(none)</option>
            {columns.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
          {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
          <select
            aria-label="sql order direction"
            className={FIELD}
            value={query.orderBy?.direction ?? "asc"}
            disabled={!query.orderBy?.column}
            onChange={(e) =>
              query.orderBy &&
              onChange({ ...query, orderBy: { ...query.orderBy, direction: e.target.value as "asc" | "desc" } })
            }
          >
            <option value="asc">asc</option>
            <option value="desc">desc</option>
          </select>
        </div>
      </Row>

      <Row label="Limit">
        <Input
          aria-label="sql limit"
          type="number"
          min={1}
          className={`${FIELD} w-24`}
          value={query.limit ?? ""}
          onChange={(e) =>
            onChange({ ...query, limit: e.target.value ? Number(e.target.value) : undefined })
          }
        />
      </Row>

      {/* Live SurrealQL preview — what Builder→Code would generate. */}
      <div className="mt-1 rounded-md border border-border bg-bg px-2 py-1">
        <span className="text-[10px] text-muted">Preview</span>
        <pre className="overflow-x-auto font-mono text-[11px] text-fg" aria-label="sql preview">
          {toSurrealQL(query) || "— pick a table —"}
        </pre>
      </div>
    </div>
  );
}

/** One filter row — column, operator, value. */
function FilterRow({
  filter,
  columns,
  onChange,
  onRemove,
}: {
  filter: SqlFilter;
  columns: string[];
  onChange: (f: SqlFilter) => void;
  onRemove: () => void;
}) {
  return (
    <div className="flex items-center gap-1">
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
      <select
        aria-label="sql filter column"
        className={FIELD}
        value={filter.column}
        onChange={(e) => onChange({ ...filter, column: e.target.value })}
      >
        {columns.map((c) => (
          <option key={c} value={c}>
            {c}
          </option>
        ))}
      </select>
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
      <select
        aria-label="sql filter operator"
        className={`${FIELD} w-16`}
        value={filter.operator}
        onChange={(e) => onChange({ ...filter, operator: e.target.value as SqlOperator })}
      >
        {OPERATORS.map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </select>
      <Input
        aria-label="sql filter value"
        className={`${FIELD} w-28`}
        value={String(filter.value)}
        onChange={(e) => onChange({ ...filter, value: e.target.value })}
      />
      <IconButton label="remove filter" onClick={onRemove}>
        <X size={12} />
      </IconButton>
    </div>
  );
}

/** A labelled builder row (label column + control column). */
function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[80px_1fr] items-start gap-2">
      <span className="pt-1.5 text-[11px] font-medium text-muted">{label}</span>
      <div>{children}</div>
    </div>
  );
}

function AddButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      aria-label={label}
      onClick={onClick}
      className="h-6 w-fit px-1.5 text-[11px] text-muted"
    >
      <Plus size={11} /> add
    </Button>
  );
}

function IconButton({
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
