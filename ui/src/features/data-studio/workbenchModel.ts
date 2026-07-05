// The Data Studio workbench's Dockview model vocabulary (data-studio-10x scope, phase 1) — the pane
// kinds, the per-pane `params` payloads, the VERSIONED persisted record, and the id mint. Pure
// data/logic (no JSX): the view (`DataStudioView`) feeds these to `dockview-react`; the whole
// serialized dock (incl. every pane's draft-cell params) is what `layout.set` persists per user, so
// a saved arrangement restores panes AND drafts, not just geometry.

import type { SerializedDockview } from "dockview-react";

import type { Cell } from "@/lib/dashboard";

/** The surface key the workbench persists under (`ui_layout:[ws, user, "data-studio"]`). */
export const DATA_STUDIO_SURFACE = "data-studio";

/** The pane components the dock can mount. `builder` is the working tab; the view kinds are the
 *  pages-as-panes registry (`workbenchPanes.ts`) — the dock treats them all the same. */
export type PaneKind = "builder" | "view";

/** A builder pane's persisted params: the working draft + the library id it was last saved as. */
export interface BuilderConfig {
  cell: Cell;
  savedAs?: string;
}

/** A pages-as-panes view pane's persisted params: the registry kind (opaque data to the dock) +
 *  the pane's own in-pane selection (open flow/rule/datasource — the pane's "URL"). */
export interface ViewPaneConfig {
  kind: string;
  sel?: string | null;
}

/** The deterministic panel id for a view pane — one pane per view kind in the first cut (pages
 *  weren't written to be multi-mounted; the menu re-activates an open pane instead). */
export function viewPaneId(kind: string): string {
  return `view:${kind}`;
}

/** The persisted layout record, VERSIONED by engine. A legacy flexlayout blob (no `engine` tag)
 *  is not migratable — the loader falls back to the default workbench and surfaces a one-time
 *  "layout was reset" notice (drafts inside old layouts are the accepted loss; the library holds
 *  anything saved). */
export interface WorkbenchRecord {
  engine: "dockview";
  model: SerializedDockview;
}

/** Wrap a serialized dock for `layout.set`. */
export function workbenchRecord(model: SerializedDockview): WorkbenchRecord {
  return { engine: "dockview", model };
}

export interface LoadedWorkbench {
  /** The serialized dock to restore, or `null` for the default (empty) workbench. */
  model: SerializedDockview | null;
  /** True when a saved layout existed but was from another engine/shape — show the reset notice. */
  wasReset: boolean;
}

/** Parse a saved `layout.get` model. Never-saved → default silently; a recognized dockview record →
 *  restore; anything else (a legacy flexlayout blob, a corrupt shape) → default + notice. */
export function loadWorkbench(saved: unknown): LoadedWorkbench {
  if (saved == null) return { model: null, wasReset: false };
  if (typeof saved === "object" && (saved as WorkbenchRecord).engine === "dockview") {
    const model = (saved as WorkbenchRecord).model;
    if (model && typeof model === "object") return { model, wasReset: false };
  }
  return { model: null, wasReset: true };
}

/** A collision-proof pane id (unique within this browser session AND across persisted reloads). */
export function mintTabId(kind: string): string {
  const rand = Math.random().toString(36).slice(2, 8);
  return `${kind}-${Date.now().toString(36)}-${rand}`;
}
