// The widget builder (widget-builder scope, "Goals") — the tool-driven "add a tile" experience that
// supersedes the v1 series-only `AddWidget`. The author: (1) picks a SOURCE by friendly label (the
// source picker over `series.list` + `ext.list` — never a raw tool name), (2) chooses a VIEW from the
// vocabulary valid for that source (read views for a read source; controls for an action; scripted
// views always available), (3) previews it live through the real bridge, and (4) clicks "Add to
// dashboard" — which builds a v2 `{view, source, action, options}` cell appended to the layout and
// persisted via `dashboard.save`. Author code (Plot/D3/JSX) is entered inline and renders sandboxed.
//
// The builder is wiring + layout; each preview/view owns its data (through the bridge). No tool name is
// ever shown to the author; the resolved `{tool,args}` lives only in the cell.

import { useMemo, useState } from "react";
import { Plus, Check } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Cell, View } from "@/lib/dashboard";
import { WidgetView } from "../views/WidgetView";
import { cellTools } from "../views/WidgetView";
import { useSourcePicker } from "./useSourcePicker";
import { SQL_SOURCE_ID, type SourceEntry } from "./sourcePicker";
import { PlotCodeField, DEFAULT_PLOT_CODE, DEFAULT_D3_CODE } from "./editors/PlotCodeField";
import { TemplateSourceField, type TemplateValue, DEFAULT_INLINE_CODE } from "./editors/TemplateSourceField";
import { SqlQueryEditor, emptySqlSource } from "./sql/SqlQueryEditor";
import type { SqlSourceState } from "./sql/query";

interface Props {
  ws: string;
  existing: Cell[];
  onAdd: (cell: Cell) => void;
  /** Whether the viewer may edit the dashboard — `mcp:dashboard.save:call` for this workspace, sourced
   *  from the session grant the shell already holds (the same source the nav gates editing on). When
   *  false the whole "Add widget" surface is hidden; the host re-check on `dashboard.save` is the
   *  authoritative backstop regardless. */
  canEdit: boolean;
  /** Edit-existing-cell mode (widget-config-vars scope, Slice 1): seed the builder fields from a cell.
   *  When set, the builder shows a title field, the source/view/options seeded from `seed`, and a
   *  "Save changes" button that calls `onSave` with the rebuilt cell (keeping the cell's `i`/geometry). */
  seed?: Cell;
  /** Edit-mode write-back — called with the rebuilt cell (same `i`/x/y/w/h as `seed`). */
  onSave?: (cell: Cell) => void;
  /** Render without the surrounding panel chrome (the edit drawer supplies its own). */
  bare?: boolean;
}

/** Find the picker entry id that produced a cell's source (for edit-mode seeding). Matches a packaged
 *  tile by its `ext:` view, the SQL source by `store.query`, else the read/action tool (+ series arg). */
export function seedEntryId(cell: Cell | undefined, entries: SourceEntry[]): string {
  if (!cell) return "";
  const view = cell.view ?? "";
  if (view.startsWith("ext:")) return entries.find((e) => e.viewKey === view)?.id ?? "";
  if (cell.source?.tool === "store.query") return SQL_SOURCE_ID;
  const tool = cell.source?.tool || cell.action?.tool;
  if (!tool) return "";
  const series = (cell.source?.args as { series?: string } | undefined)?.series;
  const match = entries.find(
    (e) =>
      (e.source?.tool === tool &&
        (series === undefined ||
          (e.source?.args as { series?: string } | undefined)?.series === series)) ||
      e.action?.tool === tool,
  );
  return match?.id ?? "";
}

// No shadcn Select/Textarea primitive exists in this repo yet (only Button/Input/Badge/Card/…), so the
// source/view pickers and the code editor use native elements with this Input-matching class. The lint
// rule is suppressed per element with this justification — generating full Radix Select/Textarea
// primitives is out of this slice's scope; the controls are token-bound and keyboard-operable.
const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

/** Read views render a source's result; control views call an action; scripted views run author code. */
const READ_VIEWS: View[] = ["chart", "stat", "gauge", "table"];
const SCRIPTED_VIEWS: View[] = ["plot", "d3", "template"];
const CONTROL_VIEWS: View[] = ["switch", "slider", "button"];

/** A fresh cell key that doesn't collide with the existing ones. */
function nextKey(existing: Cell[]): string {
  let n = existing.length + 1;
  const keys = new Set(existing.map((c) => c.i));
  while (keys.has(`w${n}`)) n += 1;
  return `w${n}`;
}

/** The views valid for a chosen source. A packaged widget tile IS its own view (the single
 *  `ext:<id>/<widget>` key — no view chooser); a write action ⇒ controls; a read source ⇒ read +
 *  scripted. */
function viewsFor(entry: SourceEntry | null): View[] {
  if (!entry) return [...READ_VIEWS, ...SCRIPTED_VIEWS, ...CONTROL_VIEWS];
  if (entry.group === "widget" && entry.viewKey) return [entry.viewKey as View];
  if (entry.writes) return [...CONTROL_VIEWS];
  return [...READ_VIEWS, ...SCRIPTED_VIEWS];
}

export function WidgetBuilder({ ws, existing, onAdd, canEdit, seed, onSave, bare }: Props) {
  const { entries, installed, loading } = useSourcePicker(ws);
  const editing = !!seed;
  const [entryId, setEntryId] = useState<string>("");
  const [view, setView] = useState<View>((seed?.view as View) ?? "chart");
  const [title, setTitle] = useState<string>(seed?.title ?? "");
  // Slice B authoring state — Plot/D3 share an inline code string; `template` has its own
  // inline-or-saved value; the SQL source has the Builder⇄Code state.
  const [plotCode, setPlotCode] = useState<string>(
    typeof seed?.options?.code === "string" ? (seed.options.code as string) : "",
  );
  const [templateValue, setTemplateValue] = useState<TemplateValue>(() =>
    seed?.options?.templateId
      ? { mode: "saved", templateId: seed.options.templateId as string }
      : typeof seed?.options?.code === "string"
        ? { mode: "inline", code: seed.options.code as string }
        : { mode: "inline", code: DEFAULT_INLINE_CODE },
  );
  const [sqlState, setSqlState] = useState<SqlSourceState>(() =>
    (seed?.options?.sql as SqlSourceState | undefined) ?? emptySqlSource(),
  );
  // In edit mode, seed the chosen source from the cell once the picker entries have loaded.
  const [seeded, setSeeded] = useState(false);
  if (editing && !seeded && entries.length > 0) {
    setEntryId(seedEntryId(seed, entries));
    setSeeded(true);
  }

  const entry = useMemo(() => entries.find((e) => e.id === entryId) ?? null, [entries, entryId]);
  const validViews = viewsFor(entry);
  const isScripted = SCRIPTED_VIEWS.includes(view);
  const isSqlSource = entry?.id === SQL_SOURCE_ID;
  // A packaged extension tile: the cell is a v2 `{ v:2, view:"ext:<id>/<widget>" }` — no source/action,
  // no view chooser (the tile owns its renderer + data). Its scope is re-checked at the host per call.
  const isWidget = entry?.group === "widget";

  // Build the candidate cell from the current selection — also the preview's input.
  const candidate: Cell | null = useMemo(() => {
    if (!entry && !isScripted) return null;
    const options: Record<string, unknown> = {};

    // Scripted-view code goes into cell.options (inline) or a templateId reference.
    if (view === "plot" || view === "d3") {
      if (plotCode.trim()) options.code = plotCode;
    } else if (view === "template") {
      if (templateValue.mode === "inline") {
        if (templateValue.code.trim()) options.code = templateValue.code;
      } else if (templateValue.templateId) {
        options.templateId = templateValue.templateId;
      }
    }

    // The SQL source resolves to `{ tool: "store.query", args: { sql } }` from the editor state, and
    // stows the full SQL source state (builder query + format) so reopening returns to the builder.
    let source = entry?.source;
    if (isSqlSource) {
      source = { tool: "store.query", args: { sql: sqlState.rawSql } };
      options.sql = sqlState;
    }

    // A packaged tile's `view` is its `ext:<id>/<widget>` key; otherwise the chosen view.
    const cellViewKey = (isWidget && entry?.viewKey ? entry.viewKey : view) as View;
    return {
      i: "preview",
      x: 0,
      y: 0,
      w: 4,
      h: 3,
      v: 2,
      // `widget_type` is the v1 fallback (chart/stat/gauge); v2 render is driven by `view`. A packaged
      // tile's `view` is its `ext:<id>/<widget>` key — `cellView` reads `view` first, so the fallback
      // stays a harmless "chart" exactly as every other v2 cell does.
      widget_type: "chart",
      view: cellViewKey,
      title: title.trim() || undefined,
      binding: { series: "" },
      source,
      action: entry?.action,
      options,
    };
  }, [entry, view, title, plotCode, templateValue, sqlState, isScripted, isSqlSource, isWidget]);

  const reset = () => {
    setPlotCode("");
    setTemplateValue({ mode: "inline", code: DEFAULT_INLINE_CODE });
    setSqlState(emptySqlSource());
    setTitle("");
    setEntryId("");
  };

  const add = () => {
    if (!candidate) return;
    const y = existing.reduce((m, c) => Math.max(m, c.y + c.h), 0);
    onAdd({ ...candidate, i: nextKey(existing), y });
    reset();
  };

  // Edit mode: rebuild the cell from the seeded fields, keeping the original key + geometry.
  const save = () => {
    if (!candidate || !seed || !onSave) return;
    onSave({ ...candidate, i: seed.i, x: seed.x, y: seed.y, w: seed.w, h: seed.h });
  };

  // Gate the whole add surface on the edit cap (rule 5: capability-first). A read-only viewer sees the
  // dashboard without any add affordance; the host re-check on `dashboard.save` is the real backstop.
  if (!canEdit) return null;

  return (
    <div
      className={bare ? "text-xs" : "border-b border-border bg-panel px-3 py-3 text-xs"}
      aria-label={editing ? "widget settings" : "widget builder"}
    >
      <div className="flex flex-wrap items-center gap-2">
        {!editing && <span className="font-medium text-muted">Add widget</span>}

        {/* Title — the cell header label (fallback to a derived label when empty). */}
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Input variant used here; see FIELD note */}
        <input
          aria-label="widget title"
          className={`${FIELD} w-44`}
          placeholder="title (optional)"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
        />

        {/* Source picker — friendly labels grouped by origin; the author never sees a tool name. */}
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive; see FIELD note */}
        <select
          aria-label="widget source"
          className={`${FIELD} w-64`}
          value={entryId}
          onChange={(e) => {
            setEntryId(e.target.value);
            const next = entries.find((x) => x.id === e.target.value) ?? null;
            const vs = viewsFor(next);
            if (!vs.includes(view)) setView(vs[0]);
          }}
        >
          <option value="">{loading ? "loading sources…" : "— pick a source —"}</option>
          <PickerGroup entries={entries} group="series" label="Series" />
          <PickerGroup entries={entries} group="live" label="Live (Zenoh)" />
          <PickerGroup entries={entries} group="sql" label="Direct SurrealDB" />
          <PickerGroup entries={entries} group="extension" label="Installed extension" />
          <PickerGroup entries={entries} group="action" label="Action (control)" />
          <PickerGroup entries={entries} group="widget" label="Extension widgets" />
        </select>

        {/* View chooser — only the views valid for the chosen source. Hidden for a packaged extension
            tile: it IS its own view (the single `ext:<id>/<widget>` key), so there is nothing to pick. */}
        {!isWidget && (
          /* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive; see FIELD note */
          <select
            aria-label="widget view"
            className={FIELD}
            value={view}
            onChange={(e) => {
              const v = e.target.value as View;
              setView(v);
              // Seed a working default snippet when an empty Plot/D3 editor first appears.
              if (v === "plot" && !plotCode.trim()) setPlotCode(DEFAULT_PLOT_CODE);
              if (v === "d3" && !plotCode.trim()) setPlotCode(DEFAULT_D3_CODE);
            }}
          >
            {validViews.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>
        )}

        {editing ? (
          <Button aria-label="save widget" size="sm" onClick={save}>
            <Check size={12} /> Save changes
          </Button>
        ) : (
          <Button aria-label="add widget" size="sm" onClick={add}>
            <Plus size={12} /> Add
          </Button>
        )}
      </div>

      {/* The SQL source: the Grafana-style Builder⇄Code editor (Slice C). Resolves to a
          `{tool:"store.query", args:{sql}}` source; the rows render in the chosen view. */}
      {isSqlSource && <SqlQueryEditor value={sqlState} onChange={setSqlState} />}

      {/* Scripted views: the ported CodeMirror editors (Slice B) — author code that renders sandboxed
          and may call a granted tool. Plot/D3 share the code editor; `template` has inline-or-saved. */}
      {(view === "plot" || view === "d3") && (
        <PlotCodeField engine={view} value={plotCode} onChange={setPlotCode} />
      )}
      {view === "template" && (
        <TemplateSourceField value={templateValue} onChange={setTemplateValue} />
      )}

      {/* Live preview through the REAL bridge — never a fake render. */}
      {candidate && (
        <div
          className="mt-2 h-40 w-64 rounded-md border border-border bg-bg p-2"
          aria-label="widget preview"
        >
          <WidgetView
            cell={{ ...candidate, i: "preview" }}
            installed={installed}
            workspace={ws}
          />
          <span className="sr-only" data-preview-tools={cellTools(candidate).join(",")} />
        </div>
      )}
    </div>
  );
}

/** Render one `<optgroup>` of the picker for a source group, empty-tolerant. */
function PickerGroup({
  entries,
  group,
  label,
}: {
  entries: SourceEntry[];
  group: SourceEntry["group"];
  label: string;
}) {
  const items = entries.filter((e) => e.group === group);
  if (items.length === 0) return null;
  return (
    <optgroup label={label}>
      {items.map((e) => (
        <option key={e.id} value={e.id}>
          {e.label}
        </option>
      ))}
    </optgroup>
  );
}
