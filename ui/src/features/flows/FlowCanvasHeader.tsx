// The flow canvas header bar (flows-canvas scope) — the primary toolbar (`FlowToolbar`) plus the
// secondary chrome (undo, export, import, delete) and the run-status / error badges. Extracted from
// FlowCanvas so the canvas file owns graph state + handlers, not the header markup (FILE-LAYOUT).
// Presentational: every action + piece of state is a prop.

import { useRef } from "react";
import { Bug, Download, RotateCcw, Trash2, Upload } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

import { FlowToolbar, type FlowToolbarProps } from "./FlowToolbar";

export interface FlowCanvasHeaderProps extends FlowToolbarProps {
  /** Undo is available (the stack is non-empty). */
  canUndo: boolean;
  /** The current run status (drives the badge), if a run is being watched. */
  runStatus: string | null;
  saveError: string | null;
  runError: string | null;
  /** Whether the debug panel drawer is open (debug-node-scope). */
  debugOpen: boolean;
  onUndo: () => void;
  onExport: () => void;
  onImport: (file: File) => void;
  onDelete: () => void;
  /** Toggle the debug panel drawer (debug-node-scope — Node-RED's debug sidebar). */
  onToggleDebug: () => void;
}

export function FlowCanvasHeader({
  canUndo,
  runStatus,
  saveError,
  runError,
  debugOpen,
  onUndo,
  onExport,
  onImport,
  onDelete,
  onToggleDebug,
  ...toolbar
}: FlowCanvasHeaderProps) {
  const importedFile = useRef<HTMLInputElement>(null);
  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-border bg-card/60 px-3 py-2">
      <FlowToolbar {...toolbar} />
      <div className="mx-1 h-5 w-px bg-border" />
      <Button aria-label="undo" onClick={onUndo} variant="ghost" size="sm" disabled={!canUndo} className="gap-1.5">
        <RotateCcw size={13} />
        Undo
      </Button>
      <Button aria-label="export flow" onClick={onExport} variant="ghost" size="sm" className="gap-1.5">
        <Download size={13} />
        Export
      </Button>
      <Button
        aria-label="import flow"
        onClick={() => importedFile.current?.click()}
        variant="ghost"
        size="sm"
        className="gap-1.5"
      >
        <Upload size={13} />
        Import
      </Button>
      {/* eslint-disable-next-line no-restricted-syntax -- a hidden native file picker; no shadcn equivalent */}
      <input
        ref={importedFile}
        type="file"
        accept="application/json"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) onImport(f);
          e.target.value = "";
        }}
      />
      <div className="ml-auto flex flex-wrap items-center gap-2">
        {/* The debug panel toggle (debug-node-scope) — Node-RED's debug-sidebar tab, always reachable
            from the header. Highlighted (default variant) when the drawer is open. */}
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
        {runStatus ? (
          <Badge
            variant="outline"
            data-status={runStatus}
            className={cn(
              "rounded-full capitalize",
              runStatus === "success" && "border-emerald-500/40 text-emerald-600 dark:text-emerald-400",
              (runStatus === "failed" || runStatus === "partialFailure") && "border-destructive/40 text-destructive",
              runStatus === "running" && "border-amber-500/50 text-amber-600 dark:text-amber-400",
            )}
            aria-label="run status"
          >
            {runStatus}
          </Badge>
        ) : null}
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
        <Button
          aria-label="delete flow"
          onClick={onDelete}
          variant="ghost"
          size="sm"
          className="gap-1.5 text-muted hover:text-destructive"
        >
          <Trash2 size={13} />
          Delete
        </Button>
      </div>
    </div>
  );
}
