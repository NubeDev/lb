// The row-list visual query builder body (widget-builder Slice C) — ported from Grafana's
// `visual-query-builder/VisualEditor.tsx`, rendered with our own primitives. The rows a non-SQL user
// fills: Table → Column/Aggregation → Filter → Group by → Order by → Limit, with a live SQL preview.
// Extracted from `VisualEditor.tsx` (visual-canvas-builder slice) so that file stays a thin host
// under the 400-line ceiling (FILE-LAYOUT). Kept byte-identical for the surreal regression gateway
// test (`aria-label="sql preview"` / `"sql visual builder"` / `"sql table"`).
//
// react-querybuilder slice: the Filter section is now `<FilterQueryBuilder>` (a pair of
// `react-querybuilder` `<QueryBuilder>` instances — WHERE + HAVING — in `independentCombinators`
// mode, projected 1:1 onto the flat `SqlFilter[]`). The rest of the chrome (Table / Columns /
// Group by / Order by / Limit / preview) is unchanged — react-querybuilder owns ONLY the boolean
// filter expression, never SELECT/JOIN/GROUP BY/ORDER BY/LIMIT.

import { Plus, X } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Schema } from "@/lib/schema";
import type { SqlAggregation, SqlBuilderQuery, SqlOrderBy } from "@/lib/panel-kit/sql/query";
import { normalizeOrderBy } from "@/lib/panel-kit/sql/query";
import { emitSql, type SqlDialect } from "@/lib/panel-kit/sql/dialect";
import { FilterQueryBuilder } from "@/features/query-builder/rules";
import { JoinRows } from "./JoinRows";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

const AGGREGATIONS: (SqlAggregation | "")[] = ["", "count", "count_distinct", "sum", "avg", "min", "max"];

interface Props {
  schema: Schema;
  query: SqlBuilderQuery;
  onChange: (q: SqlBuilderQuery) => void;
  dialect: SqlDialect;
}

/** A selectable column option: bare `column` when the query has no joins (back-compat), qualified
 *  `table.column` once joins exist (the same convention `filterQueryBuilder.ts` uses). */
interface FieldOption {
  /** The option value — `column` or `table.column`. */
  value: string;
  /** The owning table, or undefined for the FROM table / no-joins case. */
  table?: string;
  column: string;
}

/** Every pickable column across the FROM table + each joined table. */
function fieldOptions(schema: Schema, query: SqlBuilderQuery): FieldOption[] {
  const joined = !!query.joins && query.joins.length > 0;
  const tables = [query.table, ...(query.joins ?? []).map((j) => j.table)];
  const out: FieldOption[] = [];
  for (const t of tables) {
    for (const c of schema.tables.find((x) => x.name === t)?.columns ?? []) {
      out.push(
        joined
          ? { value: `${t}.${c.name}`, table: t === query.table ? undefined : t, column: c.name }
          : { value: c.name, column: c.name },
      );
    }
  }
  return out;
}

/** The historical row-list builder body — table/joins/columns/filters/groupBy/orderBy/limit + preview. */
export function VisualRows({ schema, query, onChange, dialect }: Props) {
  const tableNames = schema.tables.map((t) => t.name);
  const fields = fieldOptions(schema, query);
  const hasJoins = !!query.joins && query.joins.length > 0;
  const fieldValue = (o: { column: string; table?: string }) =>
    hasJoins ? `${o.table ?? query.table}.${o.column}` : o.column;
  const fieldByValue = (v: string) => fields.find((f) => f.value === v);

  const setTable = (table: string) =>
    // A new table invalidates column-bound clauses — reset them honestly rather than carry stale ones.
    onChange({ ...query, table, joins: undefined, columns: [], filters: [], groupBy: [], orderBy: undefined });

  const addColumn = () =>
    onChange({ ...query, columns: [...query.columns, { name: fields[0]?.column ?? "*", table: fields[0]?.table }] });

  // The rows path treats orderBy as a single clause (its UI is single-column). Write the array shape
  // (the WRITE contract); read both shapes via `normalizeOrderBy`.
  const ob0 = normalizeOrderBy(query.orderBy)?.[0];

  return (
    <div className="grid gap-2" aria-label="sql visual builder">
      {/* Table */}
      <Row label="Table">
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
        <select
          aria-label="sql table"
          className={FIELD}
          value={query.table}
          onChange={(e) => setTable(e.target.value)}
        >
          <option value="">— pick a table —</option>
          {tableNames.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
      </Row>

      {/* Joins — standard dialect only (SurrealQL has no ANSI JOIN; same gate as the canvas). */}
      {dialect === "standard" && query.table && (
        <Row label="Joins">
          <JoinRows schema={schema} query={query} onChange={onChange} />
        </Row>
      )}

      {/* Columns / aggregations */}
      <Row label="Columns">
        <div className="grid gap-1">
          {query.columns.map((col, i) => (
            <div key={i} className="flex items-center gap-1">
              {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
              <select
                aria-label={`sql column ${i}`}
                className={FIELD}
                value={col.name === "*" ? "*" : fieldValue({ column: col.name, table: col.table })}
                onChange={(e) => {
                  const next = [...query.columns];
                  const f = fieldByValue(e.target.value);
                  next[i] =
                    e.target.value === "*" || !f
                      ? { ...col, name: e.target.value, table: undefined }
                      : { ...col, name: f.column, table: f.table };
                  onChange({ ...query, columns: next });
                }}
              >
                <option value="*">*</option>
                {fields.map((f) => (
                  <option key={f.value} value={f.value}>
                    {f.value}
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
              <IconButton
                label={`remove column ${i}`}
                onClick={() => onChange({ ...query, columns: query.columns.filter((_, j) => j !== i) })}
              >
                <X size={12} />
              </IconButton>
            </div>
          ))}
          <AddButton label="add column" onClick={addColumn} />
        </div>
      </Row>

      {/* Filters (WHERE + HAVING) — react-querybuilder-backed. Projects 1:1 onto the flat
          `SqlFilter[]`; edits flatten back through `fromRuleGroup` and `emitSql` renders the SQL. */}
      <FilterQueryBuilder schema={schema} query={query} onChange={onChange} />

      {/* Group by — the multi-select carries only string entries (a `<select>` limitation). */}
      <Row label="Group by">
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive (multi via comma) */}
        <select
          aria-label="sql group by"
          multiple
          className={`${FIELD} h-16`}
          value={(query.groupBy ?? []).map((g) =>
            typeof g === "string" ? (hasJoins ? `${query.table}.${g}` : g) : fieldValue(g),
          )}
          onChange={(e) =>
            onChange({
              ...query,
              groupBy: Array.from(e.target.selectedOptions).map((o) => {
                const f = fieldByValue(o.value);
                // Bare string = a FROM-table column (back-compat); object = a joined table's.
                return f?.table ? { table: f.table, column: f.column } : f?.column ?? o.value;
              }),
            })
          }
        >
          {fields.map((f) => (
            <option key={f.value} value={f.value}>
              {f.value}
            </option>
          ))}
        </select>
      </Row>

      {/* Order by (single-column UI; writes the array shape) + Limit */}
      <Row label="Order by">
        <div className="flex items-center gap-1">
          {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
          <select
            aria-label="sql order column"
            className={FIELD}
            value={ob0?.column ? fieldValue(ob0) : ""}
            onChange={(e) => {
              const direction = ob0?.direction ?? "asc";
              const f = fieldByValue(e.target.value);
              const next: SqlOrderBy[] | undefined = f
                ? [{ column: f.column, ...(f.table ? { table: f.table } : {}), direction }]
                : undefined;
              onChange({ ...query, orderBy: next });
            }}
          >
            <option value="">(none)</option>
            {fields.map((f) => (
              <option key={f.value} value={f.value}>
                {f.value}
              </option>
            ))}
          </select>
          {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
          <select
            aria-label="sql order direction"
            className={FIELD}
            value={ob0?.direction ?? "asc"}
            disabled={!ob0?.column}
            onChange={(e) => {
              if (!ob0?.column) return;
              const direction = e.target.value as "asc" | "desc";
              onChange({ ...query, orderBy: [{ ...ob0, direction }] });
            }}
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

      {/* Live SQL preview — what Builder→Code would generate for this dialect. */}
      <div className="mt-1 rounded-md border border-border bg-bg px-2 py-1">
        <span className="text-[10px] text-muted">Preview</span>
        <pre className="overflow-x-auto font-mono text-[11px] text-fg" aria-label="sql preview">
          {emitSql(dialect, query) || "— pick a table —"}
        </pre>
      </div>
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
