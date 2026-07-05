// The INLINE panel builder (panel-builder views; data-studio-10x scope, phase 3) — a VIEW over the
// headless `usePanelEditor` state machine (`@/lib/panel-kit`), rendered to FILL ITS PARENT. Two
// arrangements (the `layout` prop):
//   - "split"   — the v2/v3 surface unchanged: preview+viz pills LEFT, the full options rail RIGHT
//     (the dashboard-parity default).
//   - "stacked" — the QUERY-FIRST flow (Data Studio): one compact toolbar (inline title, Run, ONE
//     Save split-button) + the focused query editor; NO preview / viz pills / options until rows
//     exist. Rows returned → the live preview + the `VizGallery` thumbnail cards (rendered from the
//     ONE already-fetched query) + the collapsed searchable `OptionsDrawer`. A zero-row query offers
//     "Preview with demo data" — the SHIPPED SQLite demo datasource, badged `demo`, auto-yielding
//     the moment the user's own query has rows (real records, real engine — rule 9).

import { useEffect, useMemo, useState } from "react";
import { Check, Database } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Cell } from "@/lib/dashboard";
import { cellPrimaryTarget } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { usePanelEditor, saveDraftAsPanel, slugify } from "@/lib/panel-kit";
import { refCell } from "@/lib/panel";
import { CAP, getSession, hasCap } from "@/lib/session";

import { defaultOptionsForView } from "./viewOptions";
import { VizPicker } from "./VizPicker";
import { VizGallery } from "./VizGallery";
import { QueryStatusBar } from "./QueryStatusBar";
import { PreviewToolbar } from "./PreviewToolbar";
import { BuilderToolbar } from "./BuilderToolbar";
import { DataInspector } from "./DataInspector";
import { usePanelData } from "@/features/dashboard/builder/usePanelData";
import { detectShape } from "@/features/dashboard/views/shape";
import { fieldNamesOf } from "./fields/resultFields";
import { ResultFieldsProvider } from "./fields/FieldsContext";
import { LibraryPanelBar } from "./LibraryPanelBar";
import { PreviewPane } from "./PreviewPane";
import { OptionsSections } from "./OptionsSections";
import { OptionsDrawer } from "./OptionsDrawer";
import { QueryTargets } from "./tabs/QueryTargets";
import { useDemoPreview, demoSwappedCell } from "./useDemoPreview";

interface Props {
  ws: string;
  /** The cell to edit (EDIT) or a fresh default cell (ADD). Both enter the same (de)serializer. */
  cell: Cell;
  /** Persist the built cell. The host re-checks the save verb; any UI gate is convenience only. */
  onSave: (cell: Cell) => void;
  /** Notified on every draft change (Data Studio stows the draft in its dock-pane params so the
   *  workbench survives reload). Optional — a consumer that only wants Save can omit it. */
  onDraftChange?: (cell: Cell) => void;
  scope?: VarScope;
  /** The Save button label ("Save" | "Save to tab" …) — the consumer names the persistence. */
  saveLabel?: string;
  /** "split" (preview LEFT, options RIGHT — the default) or "stacked" (the query-first flow). */
  layout?: "split" | "stacked";
}

export function BuilderPane({
  ws,
  cell,
  onSave,
  onDraftChange,
  scope = emptyScope(),
  saveLabel = "Save",
  layout = "split",
}: Props) {
  const stacked = layout === "stacked";
  const ed = usePanelEditor(cell, { defaultOptionsForView });
  const [tableView, setTableView] = useState(false);
  // Freeze (edit-without-requery): while frozen the datasource is NOT re-hit — option/source edits
  // reshape the frames already fetched. Unfreeze re-fetches once.
  const [frozen, setFrozen] = useState(false);
  const [inspecting, setInspecting] = useState(false);

  // The draft's data — ONE read through the one data hook (invariant A) feeds the shape probe, the
  // result-field names, AND the query-first staging (rows exist → reveal the visual stages).
  const data = usePanelData(ed.draft, scope, ed.refreshKey, { frozen });
  const resultFields = useMemo(() => fieldNamesOf(data.rows), [data.rows]);
  const hasTarget = !!cellPrimaryTarget(ed.draft)?.tool;
  const hasRows = !data.loading && data.rows.length > 0;

  // Demo data (stacked only): a zero-row query may swap the DISPLAYED cell to the shipped SQLite
  // demo datasource. The saved draft is untouched; auto-yields when the user's query has rows.
  const demo = useDemoPreview(ws, { hasTarget, loading: data.loading, rowCount: data.rows.length });
  const shownCell = demo.active ? demoSwappedCell(ed.draft) : ed.draft;
  // When demo is on the gallery/preview shape follows the DEMO frames; the hook de-dups to the same
  // cache entry as `data` when demo is off (identical cell → identical query key).
  const shownData = usePanelData(shownCell, scope, ed.refreshKey, { frozen });
  const shape = shownData.loading ? "unknown" : detectShape(shownData.rows);

  // Query-first staging: the visual stages (preview/gallery/options) reveal once rows EVER existed
  // this session (or demo turned on) and stay — a later zero-row edit shouldn't fold the workbench.
  const [revealed, setRevealed] = useState(false);
  useEffect(() => {
    if (hasRows || demo.active) setRevealed(true);
  }, [hasRows, demo.active]);

  const patch = ed.patch;
  // Report every serialized draft AFTER the state applies (an inline `ed.toCell()` next to `patch`
  // would serialize the pre-patch state — one edit behind).
  useEffect(() => {
    onDraftChange?.(ed.draft);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the draft is the only trigger
  }, [ed.draft]);

  // The Save split-button's library entry (mirrors LibraryPanelBar.saveAsLibrary — same verb, same
  // prompt-for-slug; the host re-checks `panel.save`). `null` without the cap: no affordance.
  const canSaveLibrary = hasCap(getSession()?.caps ?? [], CAP.panelSave);
  const saveAsLibrary = async () => {
    const title = ed.state.title?.trim() || "Panel";
    const id = window.prompt("Library panel id (permanent slug):", slugify(title));
    if (!id) return;
    const saved = await saveDraftAsPanel(ed.toCell(), id, title);
    onSave(refCell(ed.toCell(), saved.id));
  };

  // ⌘/Ctrl+Enter runs the query from anywhere in the builder (Grafana parity). Ignored while frozen
  // (nothing to fetch) or with no source.
  const onKeyDown = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && hasTarget && !frozen) {
      e.preventDefault();
      ed.run();
    }
  };

  if (stacked) {
    // ---- The query-first flow (Data Studio) ----
    return (
      <div aria-label="panel builder" className="flex h-full min-h-0 flex-col gap-3 p-3" onKeyDown={onKeyDown}>
        <DataInspector open={inspecting} onOpenChange={setInspecting} state={data} />
        <ResultFieldsProvider fields={resultFields}>
          <BuilderToolbar
            title={ed.state.title}
            onTitle={(title) => patch({ title })}
            canRun={hasTarget}
            loading={data.loading}
            onRun={ed.run}
            frozen={frozen}
            onToggleFreeze={() => setFrozen((f) => !f)}
            tableView={tableView}
            onToggleTableView={() => setTableView((v) => !v)}
            onInspect={() => setInspecting(true)}
            saveLabel={saveLabel}
            onSave={() => onSave(ed.toCell())}
            onSaveAsLibrary={canSaveLibrary ? () => void saveAsLibrary() : null}
          />
          {demo.available && !demo.active && (
            <div className="flex items-center gap-2 rounded-md border border-border bg-panel px-3 py-2 text-xs text-muted">
              <Database size={13} className="shrink-0" />
              <span>Your query returned no rows.</span>
              <Button aria-label="preview with demo data" size="sm" variant="outline" onClick={demo.enable}>
                Preview with demo data
              </Button>
            </div>
          )}
          {demo.active && (
            <div className="flex items-center gap-2 text-xs">
              <span
                aria-label="demo data badge"
                className="rounded border border-accent/40 bg-accent/10 px-1.5 py-0.5 font-medium uppercase tracking-wide text-accent"
              >
                demo
              </span>
              <span className="text-muted">
                Rendering the seeded demo dataset — turns off the moment your query returns rows.
              </span>
              <Button aria-label="turn off demo data" size="sm" variant="ghost" onClick={demo.disable}>
                Turn off
              </Button>
            </div>
          )}
          {!revealed ? (
            // Stage 1 — source picked: the focused, prefilled query editor. Nothing else yet.
            <div className="min-h-0 flex-1 overflow-y-auto" aria-label="query editor stage">
              <QueryTargets ws={ws} state={ed.state} patch={patch} onRun={ed.run} />
            </div>
          ) : (
            // Stage 2/3 — rows exist: preview on top, gallery below, options folded in the drawer.
            <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto">
              <div className="min-h-[10rem] shrink-0">
                <PreviewPane
                  cell={shownCell}
                  ws={ws}
                  scope={scope}
                  refreshKey={ed.refreshKey}
                  frozen={frozen}
                  tableView={tableView}
                />
              </div>
              <QueryStatusBar state={data} hasTarget={hasTarget} frozen={frozen} />
              {ed.flowKind ? (
                <VizPicker view={ed.viewC} onChange={ed.switchView} shape={shape} flowKind={ed.flowKind} />
              ) : (
                <VizGallery
                  cell={shownCell}
                  ws={ws}
                  scope={scope}
                  refreshKey={ed.refreshKey}
                  view={ed.viewC}
                  onChange={ed.switchView}
                  shape={shape}
                />
              )}
              <OptionsDrawer>
                <OptionsSections ws={ws} ed={ed} scope={scope} />
              </OptionsDrawer>
            </div>
          )}
        </ResultFieldsProvider>
      </div>
    );
  }

  // ---- The split arrangement (dashboard parity) — the v2/v3 surface unchanged ----
  const previewHalf = (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="flex items-center gap-2">
        <Input
          aria-label="panel title"
          className="h-8 text-sm"
          placeholder="Panel title"
          value={ed.state.title}
          onChange={(e) => patch({ title: e.target.value })}
        />
        <Button aria-label="save panel" size="sm" className="shrink-0" onClick={() => onSave(ed.toCell())}>
          <Check size={12} /> {saveLabel}
        </Button>
      </div>
      <LibraryPanelBar draft={ed.draft} onSave={onSave} />
      <PreviewToolbar
        hasTarget={hasTarget}
        loading={data.loading}
        frozen={frozen}
        onRun={ed.run}
        onToggleFreeze={() => setFrozen((f) => !f)}
        tableView={tableView}
        onToggleTableView={() => setTableView((v) => !v)}
        onInspect={() => setInspecting(true)}
      />
      <div className="h-56 shrink-0">
        <PreviewPane
          cell={ed.draft}
          ws={ws}
          scope={scope}
          refreshKey={ed.refreshKey}
          frozen={frozen}
          tableView={tableView}
        />
      </div>
      <QueryStatusBar state={data} hasTarget={hasTarget} frozen={frozen} />
      <VizPicker
        view={ed.viewC}
        onChange={ed.switchView}
        shape={data.loading ? "unknown" : detectShape(data.rows)}
        flowKind={ed.flowKind}
      />
    </div>
  );

  return (
    <div aria-label="panel builder" className="flex h-full min-h-0 flex-col" onKeyDown={onKeyDown}>
      <DataInspector open={inspecting} onOpenChange={setInspecting} state={data} />
      <ResultFieldsProvider fields={resultFields}>
        <div className="grid min-h-0 flex-1 grid-rows-[auto_1fr] gap-3 overflow-y-auto p-3 lg:grid-cols-[1.2fr_1fr] lg:grid-rows-1">
          {previewHalf}
          <OptionsSections ws={ws} ed={ed} scope={scope} />
        </div>
      </ResultFieldsProvider>
    </div>
  );
}
