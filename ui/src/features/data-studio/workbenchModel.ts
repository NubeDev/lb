// The Data Studio workbench's FlexLayout model vocabulary (data-studio scope v2) — the tab kinds, the
// per-tab `config` payloads, the default layout, and the tab-json factories. Pure data/logic (no JSX):
// the view (`DataStudioView`) feeds these to `flexlayout-react`; the whole model JSON (incl. every
// tab's draft-cell config) is what `layout.set` persists per user, so a saved arrangement restores
// tabs AND drafts, not just geometry.

import type { IJsonModel, IJsonTabNode } from "flexlayout-react";

import type { Cell, View } from "@/lib/dashboard";

/** The surface key the workbench persists under (`ui_layout:[ws, user, "data-studio"]`). */
export const DATA_STUDIO_SURFACE = "data-studio";

/** The tab components the factory can mount. `sources`/`library` are the border dock panes;
 *  `explore`/`builder` are the center working tabs (N of each, opened/closed/split freely). */
export type PaneKind = "sources" | "library" | "explore" | "builder";

/** An explore tab's persisted config: the draft cell (source + view baked in). */
export interface ExploreConfig {
  cell: Cell;
}

/** A builder tab's persisted config: the working draft + the library id it was last saved as. */
export interface BuilderConfig {
  cell: Cell;
  savedAs?: string;
}

/** The center tabset every new tab lands in by default. */
export const MAIN_TABSET_ID = "ds-main";

/** The workbench's default model: Sources + Library docked in the left border, an empty center. */
export function defaultWorkbenchModel(): IJsonModel {
  return {
    global: {
      // Rename via double-click, close everywhere, maximize a tabset, pop a tab out to a window —
      // the dockable-workbench feature set the scope names. Drag/split/dock are FlexLayout defaults.
      tabEnableRename: true,
      tabSetEnableMaximize: true,
      tabEnablePopout: true,
    },
    borders: [
      {
        type: "border",
        location: "left",
        size: 280,
        selected: 0,
        children: [
          {
            type: "tab",
            id: "ds-sources",
            name: "Sources",
            component: "sources" satisfies PaneKind,
            enableClose: false,
            enablePopout: false,
            enableRename: false,
          },
          {
            type: "tab",
            id: "ds-library",
            name: "Library",
            component: "library" satisfies PaneKind,
            enableClose: false,
            enablePopout: false,
            enableRename: false,
          },
        ],
      },
    ],
    layout: {
      type: "row",
      children: [{ type: "tabset", id: MAIN_TABSET_ID, weight: 100, children: [] }],
    },
  };
}

/** A fresh explore tab for a picked source. */
export function exploreTabJson(id: string, name: string, cell: Cell): IJsonTabNode {
  return {
    type: "tab",
    id,
    name,
    component: "explore" satisfies PaneKind,
    config: { cell } satisfies ExploreConfig,
  };
}

/** A fresh builder tab for a draft cell (a new panel, an explored source, or a library panel). */
export function builderTabJson(id: string, name: string, config: BuilderConfig): IJsonTabNode {
  return {
    type: "tab",
    id,
    name,
    component: "builder" satisfies PaneKind,
    config,
  };
}

/** The explore preview's view toggle set — the shipped `views/*` renderers, one render path. */
export const EXPLORE_VIEWS: { view: View; label: string }[] = [
  { view: "table", label: "Table" },
  { view: "timeseries", label: "Chart" },
  { view: "json", label: "JSON" },
];

/** A collision-proof tab id (unique within this browser session AND across persisted reloads). */
export function mintTabId(kind: PaneKind): string {
  const rand = Math.random().toString(36).slice(2, 8);
  return `${kind}-${Date.now().toString(36)}-${rand}`;
}
