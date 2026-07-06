// The Builder/Code toggle header (widget-builder Slice C) — ported from Grafana's `QueryHeader.tsx`.
// Two responsibilities: switch `editorMode` (Builder ⇄ Code), and CONFIRM on switch back to Builder
// (Grafana's behaviour) — because hand-edited raw SQL may not round-trip into the typed builder, so a
// Code→Builder switch can clobber the author's SQL. Builder→Code is free (it just regenerates the
// string). A `format` (Table | Time series) toggle rides here too (Grafana's "Format").
//
// Slice 2: a "Format" BUTTON (distinct from the "Format: Table|time-series" select) pretty-prints
// the raw SQL via `sql-formatter`. Gated to `dialect === "standard"` — sql-formatter has no
// SurrealQL grammar and its `sql` fallback would corrupt `table:id`/`type::`/`->` (peer-review fix).

import { Sparkles } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";
import type { SqlEditorMode, SqlFormat, SqlLang } from "@/lib/panel-kit/sql/query";

interface Props {
  mode: SqlEditorMode;
  format: SqlFormat;
  /** The SQL dialect — when `standard`, the Format button is shown; when `surreal`, it is hidden
   *  (sql-formatter corrupts SurrealQL — honest absence beats a corrupting button). */
  dialect: SqlDialect;
  /** Request a mode switch. Code→Builder is confirmed by the parent (it may clobber raw SQL). */
  onModeChange: (mode: SqlEditorMode) => void;
  onFormatChange: (format: SqlFormat) => void;
  /** Called when the user clicks the Format SQL button / hits Cmd/Ctrl+Shift+F. */
  onFormat: () => void;
  /** The Code editor's authoring language (PRQL support). The toggle renders only when the host
   *  supplies `onLangChange` (a standard-dialect Code surface — PRQL has no SurrealQL backend). */
  lang?: SqlLang;
  onLangChange?: (lang: SqlLang) => void;
}

/** The Builder/Code toggle + the format toggle + (Code mode, standard dialect only) the Format SQL button. */
export function SqlQueryHeader({
  mode,
  format,
  dialect,
  onModeChange,
  onFormatChange,
  onFormat,
  lang = "sql",
  onLangChange,
}: Props) {
  // Format SQL is gated: standard dialect only, Code mode only (Builder regenerates SQL on every
  // edit, so a hand-format would be clobbered), SQL only (sql-formatter has no PRQL grammar).
  const showFormatButton = mode === "code" && dialect === "standard" && lang !== "prql";
  // The language toggle: Code mode, standard dialect, and a host that wires it (`onLangChange`).
  const showLangSelect = mode === "code" && dialect === "standard" && !!onLangChange;
  return (
    <div className="flex items-center justify-between gap-2" aria-label="sql query header">
      <div className="flex items-center gap-1" role="tablist" aria-label="sql editor mode">
        <Toggle active={mode === "builder"} label="Builder" onClick={() => onModeChange("builder")} />
        <Toggle active={mode === "code"} label="Code" onClick={() => onModeChange("code")} />
      </div>
      <div className="flex items-center gap-2">
        {showLangSelect && (
          <div className="flex items-center gap-1">
            <span className="text-[10px] text-muted">Lang</span>
            {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive */}
            <select
              aria-label="query language"
              title="SQL runs verbatim; PRQL compiles to this source's SQL dialect at Run"
              className="h-7 rounded-md border border-border bg-bg px-2 text-[11px] text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20"
              value={lang}
              onChange={(e) => onLangChange?.(e.target.value as SqlLang)}
            >
              <option value="sql">SQL</option>
              <option value="prql">PRQL</option>
            </select>
          </div>
        )}
        {showFormatButton && (
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onFormat}
            aria-label="format sql"
            title="Format SQL (Cmd/Ctrl+Shift+F)"
            className="h-7 gap-1 px-2.5 text-[11px] text-muted"
          >
            <Sparkles className="h-3 w-3" aria-hidden="true" />
            Format
          </Button>
        )}
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
