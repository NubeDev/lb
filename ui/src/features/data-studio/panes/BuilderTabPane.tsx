// One builder tab (data-studio scope v2) — a full panel builder (`BuilderPane`: manual Query /
// Transform / Field / Overrides + the GenUI "AI widget" tab) over the headless panel-kit machine.
// N of these are open side by side for compare/debug. Saving to the library (the pane's
// `LibraryPanelBar`, `panel.save`) marks the tab; every draft edit is stowed back into the tab's
// FlexLayout config so a persisted layout restores the working draft.

import { useState } from "react";

import { BuilderPane } from "@/features/panel-builder/BuilderPane";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";

import type { BuilderConfig } from "../workbenchModel";

interface Props {
  config: BuilderConfig;
  ws: string;
  scope: VarScope;
  /** Persist a config change into this tab's FlexLayout node (draft edits + the saved-as marker). */
  onConfigChange: (config: BuilderConfig) => void;
  /** Notified when a library panel was saved from this tab (refreshes the Library pane). */
  onSavedToLibrary?: (panelId: string) => void;
}

export function BuilderTabPane({ config, ws, scope, onConfigChange, onSavedToLibrary }: Props) {
  // Seed the builder ONCE from the mount-time config. The pane owns the live editing state; the
  // config writes below are persistence-only. Feeding `config.cell` straight back into `BuilderPane`
  // would loop: draft change → config write → new cell prop → new draft identity → draft change …
  const [seed] = useState(config.cell);

  const onSave = (cell: Cell) => {
    if (cell.panelRef) {
      // A library save returns a REF cell; keep the working draft, record where it went.
      const savedAs = cell.panelRef.replace(/^panel:/, "");
      onConfigChange({ ...config, savedAs });
      onSavedToLibrary?.(savedAs);
    } else {
      onConfigChange({ ...config, cell });
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      {config.savedAs && (
        <div className="border-b border-border bg-panel px-3 py-1.5 text-xs" role="status">
          Saved as library panel <span className="font-medium">{config.savedAs}</span> — reusable on any
          dashboard (Add library panel) and at <span className="font-mono">/panel/{config.savedAs}</span>.
        </div>
      )}
      <div className="min-h-0 flex-1">
        <BuilderPane
          ws={ws}
          cell={seed}
          scope={scope}
          saveLabel="Apply"
          onSave={onSave}
          onDraftChange={(cell) => onConfigChange({ ...config, cell })}
        />
      </div>
    </div>
  );
}
