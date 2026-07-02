// The right-drawer tab host. Loads the UI list once (getUiManifest, stubbed),
// allocates one tab per UI, renders the active view full-bleed. Tab switching is
// manual — selecting a component never changes the active tab. Each UI declares
// a selection mode; `follow`/`sync` bind the view to the host's selected
// component. See ../../SDUI_UNIFIED_DESIGN.md §10.

import { useEffect, useMemo, useState } from "react";
import { useStructural } from "../lib/store";
import { Table, LayoutPanelLeft, Calendar, ListTree, Timer, Repeat, Plus, SquareDashed, Code2, Boxes, BookOpen, ScanSearch, Bot } from "lucide-react";
import type { UiEntry } from "../lib/ui/types";
import type { FlexValue } from "../lib/engine-types";
import { loadComponentsByType, type TypeScanRow } from "./componentsByType";
import { RenderWidget, type RenderCtx } from "./registry";
import { CollectionWidget } from "./CollectionWidget";
import { TreeWidget } from "./TreeWidget";
import { injectUiStyles } from "./styles";
import "./widgets"; // register text/value/button
import "./SchedulePanel"; // register schedule
import "./TimerPanel"; // register timer
import "./AlarmPanel"; // register alarms
import "./AlarmHistoryPanel"; // register alarmHistory
import "./CronPanel"; // register cron
import "./JsEditorPanel"; // register jsEditor
import "./TabbedComponents"; // register tabbedEditor
import "./InspectPanel"; // register inspect
import "./AgentChatPanel"; // register agentChat

injectUiStyles();

const ICONS: Record<string, typeof Table> = {
  table: Table,
  layout: LayoutPanelLeft,
  calendar: Calendar,
  tree: ListTree,
  timer: Timer,
  cron: Repeat,
  code: Code2,
  boxes: Boxes,
  book: BookOpen,
  inspect: ScanSearch,
  bot: Bot,
};

export interface UiTabHostProps {
  /** the active extension's UIs (one inner tab each) */
  uis: UiEntry[];
  currentParentUid: number;
  selectedUids: number[];
  onSelect: (uids: number[]) => void;
  onDrillIn?: (uid: number) => void;
  onNameContextMenu?: (uid: number, x: number, y: number) => void;
  onRowsChange?: (uids: number[]) => void;
  /** create a component of a type (matched by hint, e.g. "schedule") */
  onAddComponent?: (typeHint: string) => void;
  canGoUp?: boolean;
  onUp?: () => void;
  onSetDefault?: (componentUid: number, property: string, value: FlexValue) => void;
  onSetOverride?: (componentUid: number, property: string, value: FlexValue, duration: number) => void;
  onClearOverride?: (componentUid: number, property: string) => void;
  onCallAction?: (componentUid: number, name: string, params?: Record<string, FlexValue>) => Promise<Record<string, FlexValue>>;
  /** subscribe a prop-uid set to the live WS stream while a widget is mounted */
  onSubscribeProps?: (propUids: number[]) => () => void;
  /** navigate the canvas to a component (drill in + center/select) */
  onLocate?: (componentUid: number) => void;
  /** one-shot: open this UI tab focused on a component (canvas "Open UX"). The
   *  host only acts on it when `uiId` belongs to the active extension. */
  focusRequest?: { uiId: string; uid: number; nonce: number } | null;
}

export function UiTabHost({ uis, focusRequest, ...props }: UiTabHostProps) {
  const [activeId, setActiveId] = useState<string | null>(uis[0]?.id ?? null);

  // Keep the active tab valid as the extension (its UI set) changes.
  useEffect(() => {
    setActiveId((cur) => (uis.some((u) => u.id === cur) ? cur : uis[0]?.id ?? null));
  }, [uis]);

  // "Open UX" request: switch to the requested UI tab (if it's in this
  // extension). The component focus is forwarded into the view's ctx below.
  useEffect(() => {
    if (focusRequest && uis.some((u) => u.id === focusRequest.uiId)) setActiveId(focusRequest.uiId);
  }, [focusRequest, uis]);

  const active = useMemo(() => uis.find((u) => u.id === activeId) ?? null, [uis, activeId]);
  const manifest = { uis };

  return (
    <div
      className="ce-ui-root"
      // Keep panel keystrokes (Delete/Backspace especially) from reaching the
      // canvas — otherwise editing here would delete the selected component.
      onKeyDown={(e) => e.stopPropagation()}
      style={{ display: "flex", height: "100%", minHeight: 0 }}
    >
      {/* vertical tab strip */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 2,
          padding: 4,
          borderRight: "1px solid hsl(var(--border))",
          background: "hsl(var(--background))",
          flexShrink: 0,
        }}
      >
        {manifest.uis.map((u) => {
          const Icon = ICONS[u.icon ?? ""] ?? SquareDashed;
          const on = u.id === activeId;
          return (
            <button
              key={u.id}
              title={u.label}
              aria-label={u.label}
              onClick={() => setActiveId(u.id)}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                width: 30,
                height: 30,
                borderRadius: 4,
                border: "none",
                cursor: "pointer",
                color: on ? "hsl(var(--foreground))" : "hsl(var(--muted-foreground))",
                background: on ? "hsl(var(--border))" : "transparent",
              }}
            >
              <Icon size={16} />
            </button>
          );
        })}
      </div>
      {/* active view, full-bleed */}
      <div style={{ flex: 1, minWidth: 0, minHeight: 0 }}>
        {/* Key by the submenu id so each UI gets its own component instance —
            otherwise two same-typed views (e.g. the schedule/timer/cron
            `tabbedEditor`s) would share one instance and their open tabs would
            bleed across submenus. */}
        {active && (
          <ViewBody
            key={active.id}
            entry={active}
            uis={uis}
            onPickUi={setActiveId}
            focusRequest={focusRequest && focusRequest.uiId === active.id ? focusRequest : null}
            {...props}
          />
        )}
      </div>
    </div>
  );
}

function ViewBody({
  entry,
  uis,
  onPickUi,
  focusRequest,
  ...props
}: { entry: UiEntry; uis: UiEntry[]; onPickUi: (id: string) => void } & Omit<UiTabHostProps, "uis">) {
  const { view, selection, appliesTo } = entry;
  // follow/sync bind to the selected component; ignore/drive don't read it.
  const wantsSel = selection === "follow" || selection === "sync";
  const selUid = wantsSel ? props.selectedUids[0] : undefined;
  const selType = useStructural((s) => (selUid != null ? s.components.get(selUid)?.type : undefined));
  // When a UI targets a component type, only bind to a matching selection.
  const typeOk = !appliesTo || typeMatches(selType, appliesTo);
  const boundUid = typeOk ? selUid : undefined;

  if (appliesTo && wantsSel && boundUid == null) {
    // Empty state: one column per type-bound UI in this extension. Clicking a
    // component selects it and jumps to that type's tab.
    const typeBound = uis.filter((u) => u.appliesTo);
    return (
      <MultiTypePicker
        uis={typeBound}
        currentParentUid={props.currentParentUid}
        onSelect={props.onSelect}
        onPickUi={onPickUi}
        onAddComponent={props.onAddComponent}
      />
    );
  }

  const ctx: RenderCtx = {
    componentUid: boundUid,
    callAction: props.onCallAction,
    subscribeProps: props.onSubscribeProps,
    setValue: props.onSetDefault,
    locate: props.onLocate,
    focusUid: focusRequest?.uid,
    focusNonce: focusRequest?.nonce,
  };

  if (view.type === "collection") {
    const writesSelection = selection === "sync" || selection === "drive";
    return (
      <CollectionWidget
        currentParentUid={props.currentParentUid}
        selectedUids={props.selectedUids}
        onSelect={writesSelection ? props.onSelect : undefined}
        onDrillIn={props.onDrillIn}
        onNameContextMenu={props.onNameContextMenu}
        onRowsChange={props.onRowsChange}
        canGoUp={props.canGoUp}
        onUp={props.onUp}
        onSetDefault={props.onSetDefault}
        onSetOverride={props.onSetOverride}
        onClearOverride={props.onClearOverride}
      />
    );
  }

  if (view.type === "tree") {
    const writesSelection = selection === "sync" || selection === "drive";
    return (
      <TreeWidget
        currentParentUid={props.currentParentUid}
        selectedUids={props.selectedUids}
        onSelect={writesSelection ? props.onSelect : undefined}
        onDrillIn={props.onDrillIn}
        onLocate={props.onLocate}
      />
    );
  }

  if (view.type === "layout") {
    return (
      <div style={{ padding: 10, height: "100%", overflow: "auto" }}>
        {view.children.map((w, i) => (
          <RenderWidget key={`${w.type}:${i}`} node={w} ctx={ctx} />
        ))}
      </div>
    );
  }

  return (
    <div style={{ padding: 10, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>
      {view.type} view — not yet implemented
    </div>
  );
}

/** Match a component type to a UI's `appliesTo`. Exact, or the type's last path
 *  segment (so "vendor::timer" matches "timer" but not the "schedule" package). */
function typeMatches(componentType: string | undefined, appliesTo: string): boolean {
  if (!componentType) return false;
  const t = componentType.toLowerCase();
  const a = appliesTo.toLowerCase();
  if (t === a) return true;
  const seg = t.split(/[:/.\\]+/).filter(Boolean).pop() ?? t;
  return seg === a;
}

/** Enumerate components for a set of full types globally (all folders) by type,
 *  polling so the lists stay current. No registry service — the engine `?type=`
 *  scan IS the list. One stable hook call regardless of how many types. */
function useGlobalByTypes(fullTypes: string[], pollMs = 4000): Map<string, TypeScanRow[]> {
  const key = fullTypes.join(",");
  const [map, setMap] = useState<Map<string, TypeScanRow[]>>(() => new Map());
  useEffect(() => {
    if (!key) { setMap(new Map()); return; }
    const types = key.split(",");
    let live = true;
    const tick = async () => {
      const entries = await Promise.all(
        types.map(async (t) => [t, await loadComponentsByType(t).catch(() => [])] as const),
      );
      if (live) setMap(new Map(entries));
    };
    void tick();
    const id = setInterval(() => void tick(), pollMs);
    return () => { live = false; clearInterval(id); };
  }, [key, pollMs]);
  return map;
}

/** Empty-state: one column per type-bound UI, each listing matching components +
 *  an Add button. Clicking a component selects it and jumps to that type's tab.
 *  Columns whose UI sets `global` list that type across ALL folders (a
 *  depth-independent `?type=` scan); out-of-folder rows are tagged `elsewhere`.
 *  The scan's full type is inferred from an in-folder instance, else `fullType`. */
function MultiTypePicker({
  uis,
  currentParentUid,
  onSelect,
  onPickUi,
  onAddComponent,
}: {
  uis: UiEntry[];
  currentParentUid: number;
  onSelect: (uids: number[]) => void;
  onPickUi: (id: string) => void;
  onAddComponent?: (typeHint: string) => void;
}) {
  const components = useStructural((s) => s.components);
  const inFolder = useMemo(
    () => [...components.values()].filter((c) => c.parent === currentParentUid).sort((a, b) => a.name.localeCompare(b.name)),
    [components, currentParentUid],
  );
  // Resolve the full type for each global column: prefer an in-folder instance's
  // type (exact, self-correcting), else the descriptor's `fullType`.
  const fullTypeFor = (u: UiEntry): string | undefined =>
    inFolder.find((c) => typeMatches(c.type, u.appliesTo!))?.type ?? u.fullType;
  const globalTypes = useMemo(() => {
    const set = new Set<string>();
    for (const u of uis) {
      if (!u.global) continue;
      const ft = fullTypeFor(u);
      if (ft) set.add(ft);
    }
    return [...set];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [uis, inFolder]);
  const globalRows = useGlobalByTypes(globalTypes);

  // Merge in-folder matches with the global type-scan (dedup by uid). Rows that
  // aren't in this folder are tagged `elsewhere`.
  const mergedFor = (u: UiEntry): { uid: number; name: string; path?: string; elsewhere?: boolean }[] => {
    const local = inFolder.filter((c) => typeMatches(c.type, u.appliesTo!)).map((c) => ({ uid: c.uid, name: c.name }));
    if (!u.global) return local.sort((a, b) => a.name.localeCompare(b.name));
    const ft = fullTypeFor(u);
    const rows = ft ? globalRows.get(ft) ?? [] : [];
    const seen = new Set(local.map((r) => r.uid));
    const elsewhere = rows
      .filter((r) => !seen.has(r.uid))
      .map((r) => ({ uid: r.uid, name: r.name ?? `#${r.uid}`, path: r.path, elsewhere: true }));
    return [...local, ...elsewhere].sort((a, b) => a.name.localeCompare(b.name));
  };
  return (
    <div style={{ display: "flex", height: "100%", minHeight: 0 }}>
      {uis.map((u) => {
        const matches = mergedFor(u);
        return (
          <div key={u.id} style={{ flex: 1, minWidth: 0, borderRight: "1px solid hsl(var(--border))", display: "flex", flexDirection: "column" }}>
            <div style={{ padding: "8px 10px", color: "hsl(var(--foreground))", fontSize: 12, fontWeight: 500, borderBottom: "1px solid hsl(var(--border))", flexShrink: 0 }}>
              {u.label} <span style={{ color: "hsl(var(--muted-foreground))" }}>({matches.length})</span>
            </div>
            <div style={{ flex: 1, minHeight: 0, overflow: "auto", padding: 6, display: "flex", flexDirection: "column", gap: 4 }}>
              {matches.length === 0 ? (
                <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 11, padding: 6 }}>none here</div>
              ) : (
                matches.map((c) => (
                  <button
                    key={c.uid}
                    onClick={() => { onSelect([c.uid]); onPickUi(u.id); }}
                    title={c.elsewhere ? c.path : undefined}
                    style={{ display: "flex", alignItems: "baseline", gap: 8, padding: "6px 8px", textAlign: "left", background: "hsl(var(--card))", border: "1px solid hsl(var(--border))", borderRadius: 4, cursor: "pointer", color: "hsl(var(--foreground))", fontSize: 12 }}
                  >
                    <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{c.name}</span>
                    {c.elsewhere && (
                      <span style={{ marginLeft: "auto", flexShrink: 0, color: "hsl(var(--muted-foreground))", fontSize: 10 }} title={c.path}>
                        {c.path ? c.path.replace(/^root\/?/, "").split("/").slice(0, -1).join("/") || "elsewhere" : "elsewhere"}
                      </span>
                    )}
                  </button>
                ))
              )}
            </div>
            {onAddComponent && (
              <button
                onClick={() => onAddComponent(u.appliesTo!)}
                style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 5, margin: 6, padding: "5px 8px", background: "hsl(var(--border))", border: "1px solid hsl(var(--input))", borderRadius: 4, cursor: "pointer", color: "hsl(var(--foreground))", fontSize: 11, flexShrink: 0 }}
              >
                <Plus size={13} /> Add {u.label.toLowerCase()}
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
