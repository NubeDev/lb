// Adapted from Tabularis (github.com/TabularisDB/tabularis), Apache-2.0. Interaction design preserved;
// the data layer is rewired onto our typed SqlBuilderQuery (model-as-truth, not nodes-as-truth).
//
// The side panel for the canvas (visual-canvas-builder slice): WHERE/HAVING rows, GROUP BY (qualified
// `{table,column}` under joins), multi-column ORDER BY, LIMIT, and the live SQL preview
// (`emitSql(dialect, query)`). Each edit fires `onChange` with a new `SqlBuilderQuery` — the host
// keeps `rawSql` in sync. The per-row sub-components live in `QuerySettingsRows.tsx` (FILE-LAYOUT).

import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Schema } from "@/lib/schema";
import type { SqlBuilderQuery, SqlOrderBy } from "@/lib/panel-kit/sql/query";
import { emitSql, type SqlDialect } from "@/lib/panel-kit/sql/dialect";
import {
  FilterRow,
  GroupByRow,
  OrderByRow,
  type QualifiedColumn,
} from "./QuerySettingsRows";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2 text-[11px] text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

interface Props {
  schema: Schema;
  query: SqlBuilderQuery;
  onChange: (query: SqlBuilderQuery) => void;
  dialect: SqlDialect;
}

/** The settings sidebar + live SQL preview. Renders the rows the canvas doesn't show inline. */
export function QuerySettingsPanel({ schema, query, onChange, dialect }: Props) {
  const joined = !!query.joins && query.joins.length > 0;
  const allColumns = collectQualifiedColumns(query, schema);

  const setFilters = (filters: SqlBuilderQuery["filters"]) => onChange({ ...query, filters });
  const setGroupBy = (groupBy: SqlBuilderQuery["groupBy"]) => onChange({ ...query, groupBy });
  const setOrderBy = (orderBy: SqlOrderBy[]) => onChange({ ...query, orderBy });

  return (
    <aside
      aria-label="sql settings panel"
      className="flex w-72 shrink-0 flex-col overflow-hidden border-l border-border bg-panel"
    >
      <div className="flex items-center justify-between border-b border-border bg-bg px-3 py-2">
        <h3 className="text-xs font-semibold text-fg">Query settings</h3>
      </div>

      <div className="flex-1 overflow-y-auto">
        {/* WHERE / HAVING */}
        <section className="border-b border-border p-3">
          <SectionLabel
            label="Filters (WHERE / HAVING)"
            onAdd={() =>
              setFilters([
                ...query.filters,
                { column: firstColumn(query, schema), operator: "=", value: "" },
              ])
            }
          />
          <div className="grid gap-1.5">
            {query.filters.map((f, i) => (
              <FilterRow
                key={i}
                filter={f}
                columns={allColumns}
                showLogical={i > 0}
                onChange={(nf) => {
                  const next = [...query.filters];
                  next[i] = nf;
                  setFilters(next);
                }}
                onRemove={() => setFilters(query.filters.filter((_, j) => j !== i))}
              />
            ))}
          </div>
        </section>

        {/* GROUP BY */}
        <section className="border-b border-border p-3">
          <SectionLabel
            label="Group by"
            onAdd={() =>
              setGroupBy([...(query.groupBy ?? []), firstGroupBy(query, schema, joined)])
            }
          />
          <div className="grid gap-1.5">
            {(query.groupBy ?? []).map((g, i) => (
              <GroupByRow
                key={i}
                entry={g}
                columns={allColumns}
                onChange={(ng) => {
                  const next = [...(query.groupBy ?? [])];
                  next[i] = ng;
                  setGroupBy(next);
                }}
                onRemove={() => setGroupBy((query.groupBy ?? []).filter((_, j) => j !== i))}
              />
            ))}
          </div>
        </section>

        {/* ORDER BY */}
        <section className="border-b border-border p-3">
          <SectionLabel
            label="Order by"
            onAdd={() =>
              setOrderBy([
                ...readOrderBy(query),
                { column: firstColumn(query, schema), direction: "asc" },
              ])
            }
          />
          <div className="grid gap-1.5">
            {readOrderBy(query).map((o, i) => (
              <OrderByRow
                key={i}
                order={o}
                columns={allColumns}
                onChange={(no) => {
                  const next = [...readOrderBy(query)];
                  next[i] = no;
                  setOrderBy(next);
                }}
                onRemove={() => setOrderBy(readOrderBy(query).filter((_, j) => j !== i))}
              />
            ))}
          </div>
        </section>

        {/* LIMIT */}
        <section className="p-3">
          <div className="mb-2 text-[10px] font-semibold uppercase text-muted">Limit</div>
          <Input
            aria-label="sql limit"
            type="number"
            min={1}
            className={`${FIELD} w-full`}
            value={query.limit ?? ""}
            onChange={(e) =>
              onChange({ ...query, limit: e.target.value ? Number(e.target.value) : undefined })
            }
          />
        </section>
      </div>

      {/* Live SQL preview — same selector the row-list VisualEditor uses (the gateway test reads it). */}
      <div className="border-t border-border bg-bg px-2 py-1">
        <span className="text-[10px] text-muted">Preview</span>
        <pre className="overflow-x-auto font-mono text-[11px] text-fg" aria-label="sql preview">
          {emitSql(dialect, query) || "— pick a table —"}
        </pre>
      </div>
    </aside>
  );
}

/** Read `query.orderBy` as a flat array (handling the legacy single-object shape). */
function readOrderBy(query: SqlBuilderQuery): SqlOrderBy[] {
  if (!query.orderBy) return [];
  return Array.isArray(query.orderBy) ? query.orderBy : [query.orderBy];
}

/** First column of the FROM table (or `""`) — the default for a new filter/order row. */
function firstColumn(query: SqlBuilderQuery, schema: Schema): string {
  const t = schema.tables.find((x) => x.name === query.table);
  return t?.columns[0]?.name ?? "";
}

/** Default for a new groupBy row — `{table, column}` under joins, else a bare column string. */
function firstGroupBy(
  query: SqlBuilderQuery,
  schema: Schema,
  joined: boolean,
): string | QualifiedColumn {
  const col = firstColumn(query, schema);
  return joined ? { table: query.table, column: col } : col;
}

/** All columns across the FROM + joined tables, qualified `table.column` (the row dropdown source). */
function collectQualifiedColumns(query: SqlBuilderQuery, schema: Schema): QualifiedColumn[] {
  const tables = [query.table, ...(query.joins ?? []).map((j) => j.table)];
  const out: QualifiedColumn[] = [];
  for (const t of tables) {
    const entry = schema.tables.find((x) => x.name === t);
    if (!entry) continue;
    for (const c of entry.columns) out.push({ table: t, column: c.name });
  }
  return out;
}

function SectionLabel({ label, onAdd }: { label: string; onAdd: () => void }) {
  return (
    <div className="mb-2 flex items-center justify-between">
      <span className="text-[10px] font-semibold uppercase text-muted">{label}</span>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        aria-label={`add ${label}`}
        onClick={onAdd}
        className="h-6 px-1.5 text-[11px] text-muted"
      >
        <Plus size={11} /> add
      </Button>
    </div>
  );
}
