// The workbench's host context (data-studio-10x scope, phase 1) — what every dock-panel component
// needs from the hosting `DataStudioView`: the workspace, the var scope, the library-refresh
// callback, and the layout `touch` (Dockview's layout event can't see pane-param edits, so panes
// schedule the persist themselves after `updateParameters`). Dockview renders panels through React
// portals, so plain context reaches them. One responsibility: the context handle.

import { createContext, useContext } from "react";

import type { VarScope } from "@/lib/vars";

export interface WorkbenchContextValue {
  ws: string;
  scope: VarScope;
  /** Notified when a builder pane saves a library panel (refreshes the rail's Library tab). */
  onSavedToLibrary: (panelId: string) => void;
  /** Schedule a layout persist (pane-param edits — draft cells — bypass onDidLayoutChange). */
  touch: () => void;
}

export const WorkbenchContext = createContext<WorkbenchContextValue | null>(null);

export function useWorkbench(): WorkbenchContextValue {
  const ctx = useContext(WorkbenchContext);
  if (!ctx) throw new Error("useWorkbench outside DataStudioView");
  return ctx;
}
