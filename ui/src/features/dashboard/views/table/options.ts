// The `table` per-viz options (viz chart-types scope; names + defaults VERBATIM from Grafana's
// `public/app/plugins/panel/table/panelcfg.cue`). showHeader, cellHeight, sortBy, enablePagination,
// footer. Per-column cell display (color background, gauge cell) is `fieldConfig.custom` (deferred);
// per-column unit/decimals come from `fieldConfig` overrides via the one bridge.
//
// One responsibility: the typed `table` options + their Grafana defaults.

export type TableCellHeight = "sm" | "md" | "lg";

export interface TableSortBy {
  displayName: string;
  desc?: boolean;
}

/** Grafana's `table` Options (Phase-2 subset). */
export interface TableOptions {
  showHeader: boolean;
  cellHeight: TableCellHeight;
  enablePagination: boolean;
  sortBy: TableSortBy[];
}

/** Grafana defaults (panelcfg `*`): showHeader true, cellHeight "sm", no pagination, no sort. */
export function defaultTableOptions(): TableOptions {
  return { showHeader: true, cellHeight: "sm", enablePagination: false, sortBy: [] };
}

export function readTableOptions(options: Record<string, unknown> | undefined): TableOptions {
  const d = defaultTableOptions();
  const o = (options ?? {}) as Partial<TableOptions>;
  return {
    showHeader: o.showHeader ?? d.showHeader,
    cellHeight: o.cellHeight ?? d.cellHeight,
    enablePagination: o.enablePagination ?? d.enablePagination,
    sortBy: Array.isArray(o.sortBy) ? o.sortBy : d.sortBy,
  };
}

/** The per-row cell padding for a `cellHeight` (Grafana's sm/md/lg heights → our spacing tokens). */
export function cellHeightClass(h: TableCellHeight): string {
  return h === "lg" ? "py-2" : h === "md" ? "py-1.5" : "py-1";
}
