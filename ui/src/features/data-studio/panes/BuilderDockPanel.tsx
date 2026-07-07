// The Dockview panel wrapper for a builder tab (data-studio-10x scope, phase 1) — adapts the dock's
// panel contract (`params` in, `updateParameters` out) to the shipped `BuilderTabPane`. Draft edits
// and the saved-as marker are stowed back into the panel's params, so the persisted layout restores
// the working draft; each write also `touch()`es the layout seam (param edits don't fire Dockview's
// layout-change event). One responsibility: the dock↔builder adapter.

import type { IDockviewPanelProps } from "dockview-react";

import type { BuilderConfig } from "../workbenchModel";
import { useWorkbench } from "../workbenchContext";
import { BuilderTabPane } from "./BuilderTabPane";

export function BuilderDockPanel({ api, params }: IDockviewPanelProps<BuilderConfig>) {
  const bench = useWorkbench();
  return (
    <BuilderTabPane
      config={params}
      ws={bench.ws}
      scope={bench.scope}
      onConfigChange={(config) => {
        api.updateParameters(config);
        bench.touch();
      }}
      onSavedToLibrary={bench.onSavedToLibrary}
    />
  );
}
