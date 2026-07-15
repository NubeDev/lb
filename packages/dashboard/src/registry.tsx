// The widget registry — the cut for the shell's `WidgetHost` dispatch entanglement. The
// consumer registers a renderer per `View` id; the grid resolves a cell's view through
// `canonicalView` (so `chart` dispatches to the `timeseries` renderer) and mounts the match.
// An UNREGISTERED view renders an honest placeholder naming the id — never a crash, never a
// fabricated widget. `ext:<id>/<widget>` cells are ordinary views to the package: a shell
// registers each exact id, or the `"ext:*"` wildcard key that catches every `ext:` view.

import type { ComponentType } from "react";
import type { Cell, View } from "./dashboard.types";
import { canonicalView, cellView } from "./dashboard.types";
import type { TimeRange } from "./timerange";

/** The wildcard key catching every `ext:<id>/<widget>` view a shell mounts via federation. */
export const EXT_WILDCARD = "ext:*";

/** What every registered renderer receives. `scope` is the OPAQUE generic the consumer's
 *  variables machinery (or anything else) flows through — the package never reads it. */
export interface WidgetRenderProps<S = unknown> {
  cell: Cell;
  range?: TimeRange;
  scope?: S;
  /** Auto-refresh tick — bump to re-run read cells. Forwarded verbatim. */
  refreshKey?: number;
  /** Whether the hosting board is editable (some renderers dim controls when it is). */
  editable?: boolean;
}

export type WidgetRenderer<S = unknown> = ComponentType<WidgetRenderProps<S>>;

export interface WidgetRegistry<S = unknown> {
  /** Register a renderer for a view id (chainable). Later registrations win. */
  register(view: View | string, renderer: WidgetRenderer<S>): WidgetRegistry<S>;
  /** The renderer for a view id — exact canonical match, then the `ext:*` wildcard for
   *  `ext:` views, else `undefined` (the grid shows the honest placeholder). */
  resolve(view: View | string): WidgetRenderer<S> | undefined;
  /** The renderer for a CELL (resolves the cell's effective view first). */
  resolveCell(cell: Cell): WidgetRenderer<S> | undefined;
  /** The registered view ids (canonical spellings). */
  views(): string[];
}

/** Build a registry, optionally seeded from a `{ view: renderer }` map. */
export function createRegistry<S = unknown>(
  initial?: Record<string, WidgetRenderer<S>>,
): WidgetRegistry<S> {
  const map = new Map<string, WidgetRenderer<S>>();
  const reg: WidgetRegistry<S> = {
    register(view, renderer) {
      map.set(canonicalView(view), renderer);
      return reg;
    },
    resolve(view) {
      const id = canonicalView(view);
      const exact = map.get(id);
      if (exact) return exact;
      if (id.startsWith("ext:")) return map.get(EXT_WILDCARD);
      return undefined;
    },
    resolveCell(cell) {
      return reg.resolve(cellView(cell));
    },
    views() {
      return [...map.keys()];
    },
  };
  if (initial) for (const [view, r] of Object.entries(initial)) reg.register(view, r);
  return reg;
}

/** The honest unknown-view placeholder — what the grid renders when the registry has no
 *  renderer for a cell's view. Says exactly what is missing; never throws, never guesses. */
export function UnknownView({ view }: { view: string }) {
  return (
    <div className="lbdg-unknown" role="note" aria-label={`unknown view ${view}`}>
      <span className="lbdg-unknown-title">No renderer for “{view}”</span>
      <span className="lbdg-unknown-hint">Register one on the widget registry to render this cell.</span>
    </div>
  );
}
