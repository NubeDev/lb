// The visual query builder host (widget-builder Slice C) — ported from Grafana's
// `visual-query-builder/VisualEditor.tsx, rendered with our own primitives (no `@grafana/ui`).
//
// visual-canvas-builder slice: this is now a thin HOST that switches between two builder bodies by a
// small "Canvas / Rules" toggle:
//   - Canvas mode (default when `dialect === "standard"` AND `schema.tables.length > 0`): renders
//     `<QueryCanvas>` + `<QuerySettingsPanel>` (React-Flow drag-and-connect joins, per-column popover,
//     WHERE/HAVING side panel). Reads/writes `SqlBuilderQuery` only — the model is the source of truth.
//   - Rules mode (the historical row-list, default for surreal or an empty schema): delegates to
//     `<VisualRows>` whose Filter section is now `<FilterQueryBuilder>` (react-querybuilder, the
//     react-querybuilder slice) — kept byte-identical for the surreal regression gateway test
//     (`aria-label="sql preview"`).
//
// The dialect + schema props decide the default; the toggle lets the user override. Editing the typed
// `SqlBuilderQuery` regenerates the SQL string (via `emitSql` for the editor's `dialect`) the parent
// keeps in sync. Builder mode can ONLY generate a SELECT. The editor takes a `dialect: SqlDialect`
// and never branches on a datasource name (rule 10).
//
// Height: the root is `flex min-h-0 flex-1 flex-col` — it fills its parent when the parent provides a
// definite height (the QueryWorkbench editor split). Canvas mode carries `min-h-[420px] flex-1` so the
// React-Flow surface grows with the available space (maximise the editor → the canvas grows). Rules
// mode wraps `VisualRows` in `overflow-y-auto` so the form scrolls within the bounded height. When the
// parent has no definite height (the panel-builder QueryTab), `flex-1` is a no-op and both modes take
// their natural content height — no regression.

import { useEffect, useRef, useState } from "react";

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

/** The visual query builder host — Canvas or Rules body, switched by a small toggle. */
export function VisualEditor({ schema, query, onChange, dialect, layout, onLayoutChange }: Props) {
  // Default: Canvas when federation + schema has tables; Rules otherwise (surreal stays Rules by default).
  const canvasAvailable = dialect === "standard" && schema.tables.length > 0;
  const [uiMode, setUiMode] = useState<"canvas" | "rules">(canvasAvailable ? "canvas" : "rules");
  // The schema loads ASYNC: at mount `tables` is usually empty, so the initial default lands on
  // Rules even for a canvas-capable source. Upgrade to Canvas when availability arrives — but only
  // until the user picks a mode themselves (their choice always wins).
  const userPicked = useRef(false);
  useEffect(() => {
    if (canvasAvailable && !userPicked.current) setUiMode("canvas");
  }, [canvasAvailable]);
  const pickMode = (m: "canvas" | "rules") => {
    userPicked.current = true;
    setUiMode(m);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-2" aria-label="sql visual builder">
      <ModeToggle mode={uiMode} onChange={pickMode} canvasAvailable={canvasAvailable} />
      {uiMode === "canvas" && canvasAvailable ? (
        <div className="flex min-h-[420px] flex-1 overflow-hidden rounded-md border border-border">
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
        <div className="min-h-0 flex-1 overflow-y-auto overflow-x-hidden">
          <VisualRows schema={schema} query={query} onChange={onChange} dialect={dialect} />
        </div>
      )}
    </div>
  );
}

/** The Canvas↔Rules toggle — hidden when the canvas isn't available (surreal / empty schema). */
function ModeToggle({
  mode,
  onChange,
  canvasAvailable,
}: {
  mode: "canvas" | "rules";
  onChange: (m: "canvas" | "rules") => void;
  canvasAvailable: boolean;
}) {
  if (!canvasAvailable) return null;
  return (
    <div className="flex items-center gap-1">
      {(["canvas", "rules"] as const).map((m) => (
        <Button
          key={m}
          type="button"
          variant={mode === m ? "default" : "ghost"}
          size="sm"
          aria-label={`builder mode ${m}`}
          onClick={() => onChange(m)}
          className="h-6 px-2 text-[11px]"
        >
          {m === "canvas" ? "Canvas" : "Rules"}
        </Button>
      ))}
    </div>
  );
}
