// Generic, config-driven data table — the reusable core for homogeneous record
// collections (alarms, and any future entity list). Handles search, filters,
// column sort, multiselect + bulk actions, row actions, sticky header, empty
// state. Modeled on the wiresheet table view's mechanics; the CE component table
// (CollectionWidget) stays separate because it's heterogeneous (per-row slot
// columns). A collection becomes config of this — no bespoke panel.

import { useEffect, useMemo, useState } from "react";

export interface Column<T> {
  key: string;
  header: string;
  render: (row: T) => React.ReactNode;
  align?: "left" | "right";
  /** providing this makes the column header click-sortable */
  sortValue?: (row: T) => string | number;
}

export interface Filter<T> {
  key: string;
  /** options[].value === "all" (or "") clears the filter */
  options: { label: string; value: string }[];
  match: (row: T, value: string) => boolean;
}

export interface RowAction<T> {
  label: string;
  onClick: (row: T) => void;
  show?: (row: T) => boolean;
  danger?: boolean;
}

export interface BulkAction<T> {
  label: string;
  onClick: (rows: T[]) => void;
  danger?: boolean;
}

export interface DataTableProps<T> {
  rows: T[];
  getId: (row: T) => string;
  columns: Column<T>[];
  searchAccessor?: (row: T) => string;
  filters?: Filter<T>[];
  selectable?: boolean;
  rowActions?: RowAction<T>[];
  bulkActions?: BulkAction<T>[];
  defaultSort?: { key: string; dir: 1 | -1 };
  /** right-aligned toolbar content (e.g. Generate / Clear / New buttons) */
  toolbarRight?: React.ReactNode;
  /** content rendered between the toolbar and the table (e.g. a create form) */
  banner?: React.ReactNode;
  rowTint?: (row: T) => string | undefined;
  emptyText?: string;
}

export function DataTable<T>({
  rows,
  getId,
  columns,
  searchAccessor,
  filters,
  selectable,
  rowActions,
  bulkActions,
  defaultSort,
  toolbarRight,
  banner,
  rowTint,
  emptyText = "no rows",
}: DataTableProps<T>) {
  const [query, setQuery] = useState("");
  const [filterVals, setFilterVals] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [sort, setSort] = useState<{ key: string; dir: 1 | -1 } | null>(defaultSort ?? null);

  const view = useMemo(() => {
    const q = query.trim().toLowerCase();
    let r = rows;
    if (q && searchAccessor) r = r.filter((x) => searchAccessor(x).toLowerCase().includes(q));
    for (const f of filters ?? []) {
      const v = filterVals[f.key];
      if (v && v !== "all") r = r.filter((x) => f.match(x, v));
    }
    if (sort) {
      const col = columns.find((c) => c.key === sort.key);
      if (col?.sortValue) {
        const sv = col.sortValue;
        r = [...r].sort((a, b) => {
          const av = sv(a);
          const bv = sv(b);
          return (av < bv ? -1 : av > bv ? 1 : 0) * sort.dir;
        });
      }
    }
    return r;
  }, [rows, query, filterVals, sort, filters, columns, searchAccessor]);

  // Prune selection to ids that still exist.
  useEffect(() => {
    setSelected((s) => {
      const ids = new Set(rows.map(getId));
      let changed = false;
      const next = new Set<string>();
      for (const id of s) (ids.has(id) ? next.add(id) : (changed = true));
      return changed ? next : s;
    });
  }, [rows, getId]);

  const allSel = view.length > 0 && view.every((r) => selected.has(getId(r)));
  const toggleAll = () => setSelected(allSel ? new Set() : new Set(view.map(getId)));
  const toggle = (id: string) => setSelected((s) => { const n = new Set(s); n.has(id) ? n.delete(id) : n.add(id); return n; });
  const clickSort = (key: string) =>
    setSort((s) => (s?.key === key ? { key, dir: (s.dir === 1 ? -1 : 1) as 1 | -1 } : { key, dir: 1 }));

  const selectedRows = rows.filter((r) => selected.has(getId(r)));
  const colCount = (selectable ? 1 : 0) + columns.length + (rowActions?.length ? 1 : 0);

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column", fontSize: 12, userSelect: "none" }}>
      {/* toolbar */}
      <div style={{ display: "flex", alignItems: "center", gap: 6, padding: "5px 8px", borderBottom: "1px solid hsl(var(--border))", flexShrink: 0, flexWrap: "wrap" }}>
        {searchAccessor && (
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="search…" style={{ ...inp, flex: 1, minWidth: 120 }} />
        )}
        {(filters ?? []).map((f) => (
          <select key={f.key} value={filterVals[f.key] ?? "all"} onChange={(e) => setFilterVals((v) => ({ ...v, [f.key]: e.target.value }))} style={inp}>
            {f.options.map((o) => (
              <option key={o.value} value={o.value}>{o.label}</option>
            ))}
          </select>
        ))}
        {toolbarRight && <span style={{ marginLeft: "auto", display: "flex", gap: 6 }}>{toolbarRight}</span>}
      </div>

      {banner}

      {/* bulk bar */}
      {selectable && selected.size > 0 && (
        <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "5px 8px", borderBottom: "1px solid hsl(var(--border))", background: "hsl(var(--secondary))", flexShrink: 0 }}>
          <span style={{ color: "hsl(var(--cool))" }}>{selected.size} selected</span>
          {(bulkActions ?? []).map((b) => (
            <button key={b.label} onClick={() => { b.onClick(selectedRows); setSelected(new Set()); }} style={{ ...btn, color: b.danger ? "hsl(var(--crit))" : "hsl(var(--foreground))" }}>{b.label}</button>
          ))}
          <button onClick={() => setSelected(new Set())} style={{ ...btn, marginLeft: "auto" }}>clear</button>
        </div>
      )}

      {/* table */}
      <div style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ position: "sticky", top: 0, background: "hsl(var(--card))", color: "hsl(var(--muted-foreground))", zIndex: 1 }}>
              {selectable && <th style={{ ...th, width: 28 }}><input type="checkbox" checked={allSel} onChange={toggleAll} /></th>}
              {columns.map((c) => (
                <th
                  key={c.key}
                  onClick={c.sortValue ? () => clickSort(c.key) : undefined}
                  style={{ ...th, textAlign: c.align ?? "left", cursor: c.sortValue ? "pointer" : "default" }}
                >
                  {c.header}{sort?.key === c.key ? (sort.dir === 1 ? " ▲" : " ▼") : ""}
                </th>
              ))}
              {rowActions?.length ? <th style={{ ...th, textAlign: "right" }}>Actions</th> : null}
            </tr>
          </thead>
          <tbody>
            {view.map((r) => {
              const id = getId(r);
              return (
                <tr key={id} style={{ background: selected.has(id) ? "hsl(var(--cool) / 0.18)" : rowTint?.(r) ?? "transparent" }}>
                  {selectable && <td style={td}><input type="checkbox" checked={selected.has(id)} onChange={() => toggle(id)} /></td>}
                  {columns.map((c) => (
                    <td key={c.key} style={{ ...td, textAlign: c.align ?? "left" }}>{c.render(r)}</td>
                  ))}
                  {rowActions?.length ? (
                    <td style={{ ...td, textAlign: "right", whiteSpace: "nowrap" }}>
                      {rowActions.filter((a) => !a.show || a.show(r)).map((a) => (
                        <button key={a.label} onClick={() => a.onClick(r)} style={{ ...miniBtn, color: a.danger ? "hsl(var(--crit))" : "hsl(var(--foreground))" }}>{a.label}</button>
                      ))}
                    </td>
                  ) : null}
                </tr>
              );
            })}
            {view.length === 0 && (
              <tr><td colSpan={colCount} style={{ ...td, color: "hsl(var(--muted-foreground))", textAlign: "center", padding: 16 }}>{emptyText}</td></tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

const th: React.CSSProperties = { textAlign: "left", fontWeight: 500, padding: "5px 8px", borderBottom: "1px solid hsl(var(--border))", fontSize: 11 };
const td: React.CSSProperties = { padding: "4px 8px", borderBottom: "1px solid hsl(var(--secondary))", fontSize: 12 };
const inp: React.CSSProperties = { background: "hsl(var(--background))", color: "hsl(var(--foreground))", border: "1px solid hsl(var(--border))", borderRadius: 3, padding: "3px 6px", fontSize: 12, outline: "none" };
const btn: React.CSSProperties = { display: "flex", alignItems: "center", gap: 4, padding: "3px 9px", fontSize: 11, color: "hsl(var(--foreground))", background: "hsl(var(--border))", border: "1px solid hsl(var(--input))", borderRadius: 4, cursor: "pointer" };
const miniBtn: React.CSSProperties = { marginLeft: 4, padding: "2px 8px", fontSize: 10, color: "hsl(var(--foreground))", background: "hsl(var(--border))", border: "1px solid hsl(var(--input))", borderRadius: 3, cursor: "pointer" };
