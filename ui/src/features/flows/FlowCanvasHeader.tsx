// The flow canvas header bar (flows-canvas scope; consolidated per flow-ui-polish scope). The primary
// toolbar (`FlowToolbar`: Deploy · Run⇄Stop · Pause⇄Resume) + the Debug dock toggle + the `⋯`
// overflow menu (`FlowOverflowMenu`: Enable, Live values, Undo, Export, Import, Delete) + the
// run-status / disabled / error badges. Presentational: every action + piece of state is a prop.
//
// The flow's disabled state stays VISIBLE as a badge even though the Enable control moved into the
// overflow — safety-relevant state never hides behind a menu (flow-ui-polish "Risks").

import { Bug } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

import { FlowToolbar, type FlowToolbarProps } from "./FlowToolbar";
import { FlowOverflowMenu, type FlowOverflowMenuProps } from "./FlowOverflowMenu";

export interface FlowCanvasHeaderProps extends FlowToolbarProps, FlowOverflowMenuProps {
  saveError: string | null;
  runError: string | null;
  /** Whether the debug tab of the right dock is open (debug-node-scope). */
  debugOpen: boolean;
  /** Toggle the debug tab of the right dock (debug-node-scope — Node-RED's debug sidebar). */
  onToggleDebug: () => void;
}

export function FlowCanvasHeader({
  saveError,
  runError,
  debugOpen,
  onToggleDebug,
  enabled,
  liveValues,
  canUndo,
  onToggleEnabled,
  onToggleLiveValues,
  onUndo,
  onTransfer,
  onDelete,
  ...toolbar
}: FlowCanvasHeaderProps) {
  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-border bg-card/60 px-3 py-2">
      <FlowToolbar {...toolbar} />
      <div className="ml-auto flex flex-wrap items-center gap-2">
        {saveError ? (
          <span aria-label="flow error" className="text-xs text-destructive">
            {saveError}
          </span>
        ) : null}
        {runError ? (
          <span aria-label="run error" className="text-xs text-destructive">
            {runError}
          </span>
        ) : null}
        {!enabled ? (
          <Badge
            variant="outline"
            aria-label="flow disabled"
            className="rounded-full border-amber-500/50 text-amber-600 dark:text-amber-400"
          >
            Disabled
          </Badge>
        ) : null}
        {toolbar.runStatus ? (
          <Badge
            variant="outline"
            data-status={toolbar.runStatus}
            className={cn(
              "rounded-full capitalize",
              toolbar.runStatus === "success" &&
                "border-emerald-500/40 text-emerald-600 dark:text-emerald-400",
              (toolbar.runStatus === "failed" || toolbar.runStatus === "partialFailure") &&
                "border-destructive/40 text-destructive",
              toolbar.runStatus === "running" &&
                "border-amber-500/50 text-amber-600 dark:text-amber-400",
            )}
            aria-label="run status"
          >
            {toolbar.runStatus}
          </Badge>
        ) : null}
        {/* The debug dock toggle (debug-node-scope) — Node-RED's debug-sidebar tab, always reachable
            from the header. Highlighted (default variant) when the dock's Debug tab is open. */}
        <Button
          aria-label={debugOpen ? "close debug panel" : "open debug panel"}
          aria-pressed={debugOpen}
          onClick={onToggleDebug}
          variant={debugOpen ? "default" : "outline"}
          size="sm"
          className="gap-1.5"
          title="Debug panel (watch what each node emits)"
        >
          <Bug size={13} />
          Debug
        </Button>
        <FlowOverflowMenu
          enabled={enabled}
          liveValues={liveValues}
          canUndo={canUndo}
          onToggleEnabled={onToggleEnabled}
          onToggleLiveValues={onToggleLiveValues}
          onUndo={onUndo}
          onTransfer={onTransfer}
          onDelete={onDelete}
        />
      </div>
    </div>
  );
}
