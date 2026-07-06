// The visual query builder host (widget-builder Slice C) — ported from Grafana's
// `visual-query-builder/VisualEditor.tsx, rendered with our own primitives (no `@grafana/ui`).
//
// visual-canvas-builder slice: this is now a thin HOST that switches between two builder bodies by a
// small "Canvas / Rows" toggle:
//   - Canvas mode (default when `dialect === "standard"` AND `schema.tables.length > 0`): renders
//     `<QueryCanvas>` + `<QuerySettingsPanel>` (React-Flow drag-and-connect joins, per-column popover,
//     WHERE/HAVING side panel). Reads/writes `SqlBuilderQuery` only — the model is the source of truth.
//   - Rows mode (the historical row-list, default for surreal or an empty schema): delegates to
//     `<VisualRows>` — kept byte-identical for the surreal regression gateway test
//     (`aria-label="sql preview"`).
//
// The dialect + schema props decide the default; the toggle lets the user override. Editing the typed
// `SqlBuilderQuery` regenerates the SQL string (via `emitSql` for the editor's `dialect`) the parent
// keeps in sync. Builder mode can ONLY generate a SELECT. The editor takes a `dialect: SqlDialect`
// and never branches on a datasource name (rule 10).

import { useState } from "react";

import { Button } from "@/components/ui/button";
import type { Schema } from "@/lib/schema";
import type { SqlBuilderQuery } from "@/lib/panel-kit/sql/query";
import type { SqlDialect } from "@/lib/panel-kit/sql/dialect";
import { QueryCanvas } from "@/features/query-builder/canvas/QueryCanvas";
import { QuerySettingsPanel } from "@/features/query-builder/canvas/QuerySettingsPanel";
import { VisualRows } from "./VisualRows";

interface Props {
  schema: Schema;
  query: SqlBuilderQuery;
  onChange: (query: SqlBuilderQuery) => void;
  /** The dialect for the live preview. `surreal` for native; `standard` for federation. */
  dialect: SqlDialect;
  /** The opaque React-Flow node-position blob persisted on `SqlSourceState.builderLayout`. Consumed
   *  only by the canvas mode; the rows mode ignores it. */
  layout?: unknown;
  /** Persist a new layout blob (canvas mode calls this on node drag-stop). */
  onLayoutChange?: (layout: unknown) => void;
}

/** The visual query builder host — Canvas or Rows body, switched by a small toggle. */
export function VisualEditor({ schema, query, onChange, dialect, layout, onLayoutChange }: Props) {
  // Default: Canvas when federation + schema has tables; Rows otherwise (surreal stays Rows by default).
  const canvasAvailable = dialect === "standard" && schema.tables.length > 0;
  const [uiMode, setUiMode] = useState<"canvas" | "rows">(canvasAvailable ? "canvas" : "rows");

  return (
    <div className="grid gap-2" aria-label="sql visual builder">
      <ModeToggle mode={uiMode} onChange={setUiMode} canvasAvailable={canvasAvailable} />
      {uiMode === "canvas" && canvasAvailable ? (
        <div className="flex h-[420px] min-h-0 overflow-hidden rounded-md border border-border">
          <QueryCanvas
            schema={schema}
            query={query}
            onChange={onChange}
            layout={layout}
            onLayoutChange={onLayoutChange}
          />
          <QuerySettingsPanel schema={schema} query={query} onChange={onChange} dialect={dialect} />
        </div>
      ) : (
        <VisualRows schema={schema} query={query} onChange={onChange} dialect={dialect} />
      )}
    </div>
  );
}

/** The Canvas↔Rows toggle — hidden when the canvas isn't available (surreal / empty schema). */
function ModeToggle({
  mode,
  onChange,
  canvasAvailable,
}: {
  mode: "canvas" | "rows";
  onChange: (m: "canvas" | "rows") => void;
  canvasAvailable: boolean;
}) {
  if (!canvasAvailable) return null;
  return (
    <div className="flex items-center gap-1">
      {(["canvas", "rows"] as const).map((m) => (
        <Button
          key={m}
          type="button"
          variant={mode === m ? "default" : "ghost"}
          size="sm"
          aria-label={`builder mode ${m}`}
          onClick={() => onChange(m)}
          className="h-6 px-2 text-[11px]"
        >
          {m}
        </Button>
      ))}
    </div>
  );
}
