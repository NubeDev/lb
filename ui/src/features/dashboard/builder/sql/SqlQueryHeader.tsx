// The Builder/Code toggle header (widget-builder Slice C) — ported from Grafana's `QueryHeader.tsx`.
// Two responsibilities: switch `editorMode` (Builder ⇄ Code), and CONFIRM on switch back to Builder
// (Grafana's behaviour) — because hand-edited raw SQL may not round-trip into the typed builder, so a
// Code→Builder switch can clobber the author's SQL. Builder→Code is free (it just regenerates the
// string). A `format` (Table | Time series) toggle rides here too (Grafana's "Format").

import { Button } from "@/components/ui/button";
import type { SqlEditorMode, SqlFormat } from "./query";

interface Props {
  mode: SqlEditorMode;
  format: SqlFormat;
  /** Request a mode switch. Code→Builder is confirmed by the parent (it may clobber raw SQL). */
  onModeChange: (mode: SqlEditorMode) => void;
  onFormatChange: (format: SqlFormat) => void;
}

/** The Builder/Code toggle + the format toggle. */
export function SqlQueryHeader({ mode, format, onModeChange, onFormatChange }: Props) {
  return (
    <div className="flex items-center justify-between gap-2" aria-label="sql query header">
      <div className="flex items-center gap-1" role="tablist" aria-label="sql editor mode">
        <Toggle active={mode === "builder"} label="Builder" onClick={() => onModeChange("builder")} />
        <Toggle active={mode === "code"} label="Code" onClick={() => onModeChange("code")} />
      </div>
      <div className="flex items-center gap-1">
        <span className="text-[10px] text-muted">Format</span>
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
        <select
          aria-label="sql format"
          className="h-7 rounded-md border border-border bg-bg px-2 text-[11px] text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20"
          value={format}
          onChange={(e) => onFormatChange(e.target.value as SqlFormat)}
        >
          <option value="table">Table</option>
          <option value="time-series">Time series</option>
        </select>
      </div>
    </div>
  );
}

function Toggle({
  active,
  label,
  onClick,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      className={`h-7 px-2.5 text-[11px] ${active ? "bg-accent/15 text-fg" : "text-muted"}`}
    >
      {label}
    </Button>
  );
}
