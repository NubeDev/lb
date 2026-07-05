// The Data Studio workbench's FlexLayout model vocabulary (data-studio scope v2) — the tab kinds, the
// per-tab `config` payloads, the default layout, and the tab-json factories. Pure data/logic (no JSX):
// the view (`DataStudioView`) feeds these to `flexlayout-react`; the whole model JSON (incl. every
// tab's draft-cell config) is what `layout.set` persists per user, so a saved arrangement restores
// tabs AND drafts, not just geometry.

import type { IJsonModel, IJsonTabNode } from "flexlayout-react";

import type { Cell } from "@/lib/dashboard";

/** The surface key the workbench persists under (`ui_layout:[ws, user, "data-studio"]`). */
export const DATA_STUDIO_SURFACE = "data-studio";

/** The tab components the factory can mount. `builder` is the ONE tab kind (v3: the read-only
 *  `explore` kind is retired; the v2 `sources`/`library` border-dock panes moved into the shell-chrome
 *  `StudioRail` so the studio's left rail matches every other surface). N builders open/close/split. */
export type PaneKind = "builder";

/** A builder tab's persisted config: the working draft + the library id it was last saved as. */
export interface BuilderConfig {
  cell: Cell;
  savedAs?: string;
}

/** The center tabset every new tab lands in by default. */
export const MAIN_TABSET_ID = "ds-main";

/** The workbench's default model: an empty center tabset. Sources + Library live in the studio's
 *  left rail (`StudioRail`), NOT the dock — the dock holds only builder tabs. */
export function defaultWorkbenchModel(): IJsonModel {
  return {
    global: {
      // Rename via double-click, close everywhere, maximize a tabset, pop a tab out to a window —
      // the dockable-workbench feature set the scope names. Drag/split/dock are FlexLayout defaults.
      tabEnableRename: true,
      tabSetEnableMaximize: true,
      tabEnablePopout: true,
    },
    layout: {
      type: "row",
      children: [{ type: "tabset", id: MAIN_TABSET_ID, weight: 100, children: [] }],
    },
  };
}

/** A fresh builder tab for a draft cell (a new panel, a picked source, or a library panel). */
export function builderTabJson(id: string, name: string, config: BuilderConfig): IJsonTabNode {
  return {
    type: "tab",
    id,
    name,
    // The full title as the hover tooltip — the tab strip caps + ellipsizes long names (see
    // datastudio-dock.css), and FlexLayout's `title` attr comes from `helpText`, not `name`.
    helpText: name,
    component: "builder" satisfies PaneKind,
    config,
  };
}

/** A collision-proof tab id (unique within this browser session AND across persisted reloads). */
export function mintTabId(kind: PaneKind): string {
  const rand = Math.random().toString(36).slice(2, 8);
  return `${kind}-${Date.now().toString(36)}-${rand}`;
}
