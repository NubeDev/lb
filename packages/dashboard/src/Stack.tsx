// The mobile stack renderer — the SAME `cells[]` record as a single-column, read-only stack in
// y,x order (two presentations of one record; the grid degrades to this below its breakpoint).
// Rows become plain section dividers; a collapsed row still hides its members (`visibleCells`).
// No drag, no chrome, no persistence — a phone reads a board, it doesn't author one.

import type { Cell } from "./dashboard.types";
import { cellLabel, cellView, GRID_ROW_PX } from "./dashboard.types";
import { isRow, rowMembers, visibleCells } from "./rows";
import type { WidgetRegistry } from "./registry";
import { UnknownView } from "./registry";
import type { TimeRange } from "./timerange";

export interface DashboardStackProps<S = unknown> {
  cells: Cell[];
  registry: WidgetRegistry<S>;
  range?: TimeRange;
  scope?: S;
  refreshKey?: number;
}

export function DashboardStack<S = unknown>({
  cells,
  registry,
  range,
  scope,
  refreshKey,
}: DashboardStackProps<S>) {
  // Reading order: top-to-bottom, then left-to-right — the order the grid reads at full width.
  const ordered = [...visibleCells(cells)].sort((a, b) => a.y - b.y || a.x - b.x);
  return (
    <div className="lbdg-stack" aria-label="dashboard stack">
      {ordered.map((c) =>
        isRow(c) ? (
          <div key={c.i} className="lbdg-stack-row" aria-label={`row ${cellLabel(c)}`}>
            <span className="lbdg-row-title">{cellLabel(c)}</span>
            {rowMembers(cells, c).length > 0 && (
              <span className="lbdg-row-count">· {rowMembers(cells, c).length}</span>
            )}
          </div>
        ) : (
          <div
            key={c.i}
            className={c.transparent ? "lbdg-cell lbdg-cell--transparent" : "lbdg-cell lbdg-cell--framed"}
            style={{ minHeight: `${c.h * GRID_ROW_PX}px` }}
            aria-label={`cell ${c.i}`}
          >
            <div className="lbdg-cell-body">
              {(() => {
                const Renderer = registry.resolveCell(c);
                return Renderer ? (
                  <Renderer cell={c} range={range} scope={scope} refreshKey={refreshKey} editable={false} />
                ) : (
                  <UnknownView view={cellView(c)} />
                );
              })()}
            </div>
          </div>
        ),
      )}
    </div>
  );
}
