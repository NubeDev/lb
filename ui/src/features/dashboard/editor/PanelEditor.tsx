// The ONE Grafana-style panel editor (viz panel-editor scope) — the single surface for ADD and EDIT.
// It mounts on a cell, reconstructs the COMPLETE option model via the one `cellToEditorState`
// (de)serializer, and renders: a live preview + the viz picker + the options rail (Query / Transform /
// Panel options / Field / Overrides) with an options search. Save serializes back via
// `editorStateToCell`, PRESERVING the cell key + geometry, and calls `onSave` (→ `dashboard.save`). Add
// and edit are the SAME component, the SAME path — so the option surface and the round-trip are provably
// identical and the user's "edit loses my SQL options / add ≠ edit" bug is impossible.
//
// One responsibility: the editor shell + state + dispatch. Each tab, the picker, the preview, the
// (de)serializer live in their own files (FILE-LAYOUT); this composes them.

import { useMemo, useState } from "react";
import { Check } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from "@/components/ui/sheet";
import type { Cell, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { cellToEditorState, editorStateToCell, type EditorState } from "./cellEditorState";
import { defaultOptionsForView } from "./viewOptions";
import { EditorTabs } from "./Tabs";
import { VizPicker } from "./VizPicker";
import { OptionsSearch } from "./OptionsSearch";
import { PreviewPane } from "./PreviewPane";
import { QueryTab } from "./tabs/QueryTab";
import { TransformTab } from "./tabs/TransformTab";
import { PanelOptionsTab } from "./tabs/PanelOptionsTab";
import { FieldTab } from "./tabs/FieldTab";
import { OverridesTab } from "./tabs/OverridesTab";

interface Props {
  ws: string;
  /** The cell to edit (EDIT) or a fresh default cell (ADD). Both enter the same (de)serializer. */
  cell: Cell;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Persist the edited cell (the parent splices it into the layout + saves the whole dashboard). The
   *  host re-checks `dashboard.save`; the editor's mere presence is gated on the edit cap by the caller. */
  onSave: (cell: Cell) => void;
  scope?: VarScope;
}

const TAB_IDS = ["query", "transform", "options", "field", "overrides"] as const;
type TabId = (typeof TAB_IDS)[number];

export function PanelEditor({ ws, cell, open, onOpenChange, onSave, scope = emptyScope() }: Props) {
  // The whole working state — rebuilt from the cell via the ONE (de)serializer. Re-seeded when the
  // edited cell identity changes (a new Add/Edit target).
  const [state, setState] = useState<EditorState>(() => cellToEditorState(cell));
  const [seededFor, setSeededFor] = useState(cell.i);
  if (open && seededFor !== cell.i) {
    setState(cellToEditorState(cell));
    setSeededFor(cell.i);
  }
  const [tab, setTab] = useState<TabId>("query");
  const [search, setSearch] = useState("");
  // A debounce-ish tick so the preview re-queries when the source changes (cheap: state edits are rare).
  const refreshKey = useMemo(() => JSON.stringify(state.targets).length + (state.sql?.rawSql.length ?? 0), [state.targets, state.sql]);

  const patch = (next: Partial<EditorState>) => setState((s) => ({ ...s, ...next }));
  // The view canonicalized for DISPLAY (picker highlight + per-view tab branching). The stored
  // `state.view` stays raw so a v2 `chart` cell serializes byte-identical; here we show it as its
  // canonical `timeseries`. A new/empty cell defaults to `timeseries`.
  const viewC = canonicalView((state.view || "timeseries") as View);
  const stateC = { ...state, view: viewC };

  // Switching the viz preserves targets/fieldConfig/transformations; only the per-view `options` reset
  // to that view's Grafana defaults (panel-editor scope, Risks: "viz switch must preserve compatible
  // state"). The carry-over set is exactly: targets, sql, fieldConfig, transformations, title.
  const switchView = (view: View) =>
    setState((s) => ({ ...s, view, options: defaultOptionsForView(view) }));

  // The draft cell = what save would persist (also the preview's input). Built from the SAME serializer.
  const draft = useMemo(() => editorStateToCell(state, cell), [state, cell]);

  const save = () => {
    onSave(editorStateToCell(state, cell));
    onOpenChange(false);
  };

  const tabs = [
    { id: "query", label: "Query" },
    { id: "transform", label: "Transform", badge: state.transformations.length || undefined },
    { id: "options", label: "Panel options" },
    { id: "field", label: "Field" },
    { id: "overrides", label: "Overrides", badge: state.fieldConfig?.overrides?.length || undefined },
  ];

  return (
    <Sheet open={open} onOpenChange={onOpenChange}>
      <SheetContent side="right" className="flex w-full flex-col gap-0 overflow-hidden p-0 sm:max-w-3xl" aria-label="panel editor">
        <SheetHeader className="border-b border-border px-4 py-3">
          <SheetTitle>Edit panel</SheetTitle>
          <SheetDescription>One editor for add and edit — the full Grafana option surface.</SheetDescription>
        </SheetHeader>

        <div className="grid min-h-0 flex-1 grid-rows-[auto_1fr] gap-3 overflow-y-auto p-4 lg:grid-cols-[1.2fr_1fr] lg:grid-rows-1">
          {/* Left: live preview + the viz picker (what the panel will look like). */}
          <div className="flex min-h-0 flex-col gap-3">
            <Input
              aria-label="panel title"
              className="h-8 text-sm"
              placeholder="Panel title"
              value={state.title}
              onChange={(e) => patch({ title: e.target.value })}
            />
            <div className="h-56 shrink-0">
              <PreviewPane cell={draft} ws={ws} scope={scope} refreshKey={refreshKey} />
            </div>
            <VizPicker view={viewC} onChange={switchView} />
          </div>

          {/* Right: the options rail — search + tabs + the active tab body. */}
          <div className="flex min-h-0 flex-col gap-2">
            <OptionsSearch value={search} onChange={setSearch} />
            <EditorTabs tabs={tabs} active={tab} onSelect={(id) => setTab(id as TabId)} />
            <div className="min-h-0 flex-1 overflow-y-auto">
              {tab === "query" && <QueryTab ws={ws} state={state} patch={patch} />}
              {tab === "transform" && <TransformTab state={state} patch={patch} />}
              {tab === "options" && <PanelOptionsTab state={stateC} patch={patch} />}
              {tab === "field" && <FieldTab state={stateC} patch={patch} />}
              {tab === "overrides" && <OverridesTab state={state} patch={patch} />}
            </div>
          </div>
        </div>

        <div className="flex items-center justify-end gap-2 border-t border-border px-4 py-3">
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button aria-label="save panel" size="sm" onClick={save}>
            <Check size={12} /> Save
          </Button>
        </div>
      </SheetContent>
    </Sheet>
  );
}
