// The `collection` widget — a live table of the current folder's components,
// mirroring the legacy ComponentTable's column model: each component is a row;
// its props align into per-slot columns under Inputs / Outputs / Config group
// headers, self-labelling cells, live values. Selection is `sync` (canvas ⇄ row,
// with ctrl-toggle and shift-range). This is the root-ext version we grow toward
// the legacy table's full feature set (compare via the Legacy/UI-tabs toggle).
//
// Not yet ported (next increments): inline default-edit + right-click override,
// OVR badge, linked/hidden/exposed icons.

import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { wiresheetPortalRoot } from "../lib/portal";
import { useStructural, useValues, useStatusFlags, propertyDataType } from "../lib/store";
import {
  ROLE_NORMAL,
  CATEGORY_INPUT,
  CATEGORY_OUTPUT,
  CATEGORY_CONFIG,
  STATUS_OVERRIDDEN,
  DATATYPE_NUMBER,
  DATATYPE_STRING,
} from "../lib/engine-types";
import { facetFor, rawFacet, exposedPorts, type ComponentFacet } from "../lib/facet";
import { withChoices } from "../lib/choices";
import { fmtValueFacet } from "../lib/format";
import { FolderUp, ArrowUp, ArrowDown, Eye, EyeOff, Link2, CornerDownRight } from "lucide-react";
import type { Component, FlexValue } from "../lib/engine-types";
import { DefaultEditor, OverrideEditor } from "./valueEditors";
import { CopyButton } from "./CopyButton";

export interface CollectionWidgetProps {
  currentParentUid: number;
  selectedUids: number[];
  /** selection "sync"/"drive" → set host selection; omit for read-only */
  onSelect?: (uids: number[]) => void;
  onDrillIn?: (uid: number) => void;
  onNameContextMenu?: (uid: number, x: number, y: number) => void;
  /** report the visible component uids so the host keeps them subscribed */
  onRowsChange?: (uids: number[]) => void;
  canGoUp?: boolean;
  onUp?: () => void;
  onSetDefault?: (componentUid: number, property: string, value: FlexValue) => void;
  onSetOverride?: (componentUid: number, property: string, value: FlexValue, duration: number) => void;
  onClearOverride?: (componentUid: number, property: string) => void;
}

interface Cell {
  uid: number;
  /** property name (key) — what override/default APIs address */
  name: string;
  label: string;
  category: number;
  exposed?: boolean;
  hidden?: boolean;
  linked?: boolean;
}
interface RowData {
  c: Component;
  inputs: Cell[];
  outputs: Cell[];
  config: Cell[];
}

/** One live value cell — subscribes to its own uid so a value tick re-renders
 *  just this cell (the per-row pattern from FunctionBlock). Self-labelling. */
interface ValueCellProps {
  comp: Component;
  cell: Cell | undefined;
  groupStart: boolean;
  isEditing: boolean;
  onStartEdit: (compUid: number, uid: number) => void;
  onCommitDefault: (compUid: number, name: string, value: FlexValue) => void;
  onCancelEdit: () => void;
  onOpenOverride: (compUid: number, cell: Cell, rect: DOMRect) => void;
}

const ValueCell = memo(function ValueCell({
  comp,
  cell,
  groupStart,
  isEditing,
  onStartEdit,
  onCommitDefault,
  onCancelEdit,
  onOpenOverride,
}: ValueCellProps) {
  const uid = cell?.uid;
  const v = useValues((s) => (uid != null ? s.values.get(uid) : undefined));
  const flags = useStatusFlags((s) => (uid != null ? s.flags.get(uid) : undefined));
  const [readRect, setReadRect] = useState<DOMRect | null>(null);
  useEffect(() => {
    if (!readRect) return;
    const onDown = (ev: PointerEvent) => {
      if (!document.getElementById("ct-read-popup")?.contains(ev.target as Node)) setReadRect(null);
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
  }, [readRect]);
  if (!cell) return <td style={{ ...td, borderLeft: groupStart ? groupBorder : undefined }} />;
  const facet = facetFor(comp.uid, rawFacet(comp.properties));
  const facetEntry = withChoices(facet.get(cell.uid), comp.type, cell.name);
  const dataType = propertyDataType.get(cell.uid) ?? DATATYPE_NUMBER;
  const text = fmtValueFacet(v, dataType, facetEntry);
  const ovr = ((flags ?? 0) & STATUS_OVERRIDDEN) !== 0;
  // Outputs are computed; exposed ports edit via the child — both read-only here.
  const editable = cell.category !== CATEGORY_OUTPUT && !cell.exposed;
  // Read-only strings (outputs) open a popup so long values can be read.
  const isStr = dataType === DATATYPE_STRING || typeof v === "string";
  const full = typeof v === "string" ? v : text;

  return (
    <td
      style={{ ...td, borderLeft: groupStart ? groupBorder : undefined }}
      onContextMenu={(e) => {
        if (cell.exposed) return;
        e.preventDefault();
        e.stopPropagation();
        onOpenOverride(comp.uid, cell, e.currentTarget.getBoundingClientRect());
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", lineHeight: 1.2 }}>
        <span style={{ display: "flex", alignItems: "center", gap: 3, fontSize: 9, color: cell.exposed ? "hsl(var(--amber) / 0.4)" : "hsl(var(--muted-foreground))" }}>
          {cell.exposed && <CornerDownRight size={9} />}
          {cell.linked && <Link2 size={9} color="hsl(var(--cool))" />}
          {cell.hidden && <EyeOff size={9} />}
          {cell.label}
        </span>
        {isEditing ? (
          <DefaultEditor
            initial={v}
            dataType={dataType}
            facet={facetEntry}
            onCommit={(val) => onCommitDefault(comp.uid, cell.name, val)}
            onCancel={onCancelEdit}
          />
        ) : (
          <span
            onClick={
              editable
                ? (e) => { e.stopPropagation(); onStartEdit(comp.uid, cell.uid); }
                : isStr
                ? (e) => { e.stopPropagation(); setReadRect(e.currentTarget.getBoundingClientRect()); }
                : undefined
            }
            title={editable ? "click to edit default · right-click to override" : isStr ? "click to read · right-click to override" : "right-click to override"}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              color: "hsl(var(--foreground))",
              fontVariantNumeric: "tabular-nums",
              cursor: editable ? "pointer" : isStr ? "zoom-in" : "default",
              borderBottom: editable ? "1px dotted hsl(var(--input))" : undefined,
              maxWidth: 150,
            }}
          >
            <span title={text} style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{text || "—"}</span>
            {ovr && <OvrBadge />}
          </span>
        )}
      </div>
      {readRect &&
        createPortal(
          <div
            id="ct-read-popup"
            onClick={(e) => e.stopPropagation()}
            onPointerDown={(e) => e.stopPropagation()}
            style={{
              position: "fixed",
              left: Math.min(readRect.left, window.innerWidth - 300),
              top: Math.min(readRect.bottom + 4, window.innerHeight - 240),
              zIndex: 200,
              width: 280,
              maxHeight: 240,
              overflow: "auto",
              background: "hsl(var(--card))",
              border: "1px solid hsl(var(--border))",
              borderRadius: 6,
              boxShadow: "0 8px 24px rgba(0,0,0,0.6)",
              padding: 8,
            }}
          >
            <div style={{ display: "flex", justifyContent: "flex-end", marginBottom: 6 }}>
              <CopyButton text={typeof full === "string" ? full : String(full ?? "")} />
            </div>
            <div style={{ whiteSpace: "pre-wrap", wordBreak: "break-word", color: "hsl(var(--foreground))", fontFamily: "var(--font-mono)", fontSize: 11 }}>
              {full || "—"}
            </div>
          </div>,
          wiresheetPortalRoot(),
        )}
    </td>
  );
});

function OvrBadge() {
  return (
    <span
      title="Overridden"
      style={{
        fontSize: 8,
        fontWeight: 700,
        letterSpacing: 0.5,
        color: "hsl(var(--card))",
        background: "hsl(var(--amber))",
        borderRadius: 2,
        padding: "0 3px",
        lineHeight: "12px",
      }}
    >
      OVR
    </span>
  );
}

export function CollectionWidget({
  currentParentUid,
  selectedUids,
  onSelect,
  onDrillIn,
  onNameContextMenu,
  onRowsChange,
  canGoUp,
  onUp,
  onSetDefault,
  onSetOverride,
  onClearOverride,
}: CollectionWidgetProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const components = useStructural((s) => s.components);
  const linkedProps = useStructural((s) => s.linkedProps);
  const [query, setQuery] = useState("");
  const [dir, setDir] = useState<1 | -1>(1);
  const [showHidden, setShowHidden] = useState(false);
  const [anchor, setAnchor] = useState<number | null>(null);
  const [defaultEdit, setDefaultEdit] = useState<{ comp: number; uid: number } | null>(null);
  const [override, setOverride] = useState<{ comp: number; cell: Cell; rect: DOMRect } | null>(null);

  const startEdit = useCallback((comp: number, uid: number) => setDefaultEdit({ comp, uid }), []);
  const cancelEdit = useCallback(() => setDefaultEdit(null), []);
  const commitDefault = useCallback(
    (comp: number, name: string, value: FlexValue) => {
      onSetDefault?.(comp, name, value);
      setDefaultEdit(null);
    },
    [onSetDefault],
  );
  const openOverride = useCallback(
    (comp: number, cell: Cell, rect: DOMRect) => setOverride({ comp, cell, rect }),
    [],
  );

  const allRows = useMemo(
    () => [...components.values()].filter((c) => c.parent === currentParentUid),
    [components, currentParentUid],
  );
  const facets = useMemo(
    () => new Map<number, ComponentFacet>(allRows.map((c) => [c.uid, facetFor(c.uid, rawFacet(c.properties))])),
    [allRows],
  );

  const { rowsData, orphanStart } = useMemo<{ rowsData: RowData[]; orphanStart: number }>(() => {
    const q = query.trim().toLowerCase();
    const matches = (c: Component) =>
      !q || (c.name || c.type).toLowerCase().includes(q) || c.type.toLowerCase().includes(q);
    const cellsFor = (c: Component): RowData => {
      const facet = facets.get(c.uid);
      const buckets: Record<number, (Cell & { order: number })[]> = {
        [CATEGORY_INPUT]: [],
        [CATEGORY_OUTPUT]: [],
        [CATEGORY_CONFIG]: [],
      };
      for (const [name, p] of Object.entries(c.properties)) {
        if ((p.systemRole ?? ROLE_NORMAL) !== ROLE_NORMAL) continue;
        const f = facet?.get(p.uid);
        const linked = linkedProps.has(p.uid);
        const hidden = !!f?.hidden && !linked;
        if (!showHidden && hidden) continue;
        buckets[p.category]?.push({ uid: p.uid, name, label: f?.label || name, category: p.category, order: f?.order ?? 1e9, hidden, linked });
      }
      if (facet) {
        for (const ep of exposedPorts(facet)) {
          if (!showHidden && ep.facet.hidden) continue;
          const cat = ep.side === "input" ? CATEGORY_INPUT : CATEGORY_OUTPUT;
          buckets[cat].push({
            uid: ep.childUid,
            name: ep.facet.label || `#${ep.childUid}`,
            label: ep.facet.label || `#${ep.childUid}`,
            category: cat,
            order: ep.facet.order ?? 1e9,
            exposed: true,
            hidden: !!ep.facet.hidden,
            linked: linkedProps.has(ep.childUid),
          });
        }
      }
      const sortB = (arr: (Cell & { order: number })[]) =>
        arr.sort((a, b) => a.order - b.order || a.label.localeCompare(b.label));
      return {
        c,
        inputs: sortB(buckets[CATEGORY_INPUT]),
        outputs: sortB(buckets[CATEGORY_OUTPUT]),
        config: sortB(buckets[CATEGORY_CONFIG]),
      };
    };
    const byName = (a: Component, b: Component) => (a.name || a.type).localeCompare(b.name || b.type) * dir;
    const main = allRows.filter(matches).sort(byName);
    // Selected rows filtered out by the search are pinned at the bottom so the
    // selection never silently vanishes.
    const selSet = new Set(selectedUids);
    const orphans = q ? allRows.filter((c) => selSet.has(c.uid) && !matches(c)).sort(byName) : [];
    return { rowsData: [...main, ...orphans].map(cellsFor), orphanStart: main.length };
  }, [allRows, facets, linkedProps, query, dir, showHidden, selectedUids]);

  const { maxIn, maxOut, maxCfg } = useMemo(() => {
    let i = 0, o = 0, g = 0;
    for (const r of rowsData) {
      i = Math.max(i, r.inputs.length);
      o = Math.max(o, r.outputs.length);
      g = Math.max(g, r.config.length);
    }
    return { maxIn: i, maxOut: o, maxCfg: g };
  }, [rowsData]);

  const orderedUids = useMemo(() => rowsData.map((r) => r.c.uid), [rowsData]);
  const sel = new Set(selectedUids);

  // Report visible components so the host keeps their values subscribed.
  const allRowUids = useMemo(() => allRows.map((c) => c.uid), [allRows]);
  useEffect(() => {
    onRowsChange?.(allRowUids);
    return () => onRowsChange?.([]);
  }, [allRowUids, onRowsChange]);

  // Scroll the selected row into view when selection changes elsewhere (canvas).
  const firstSel = selectedUids[0];
  useEffect(() => {
    if (firstSel == null) return;
    scrollRef.current
      ?.querySelector<HTMLElement>(`[data-uid="${firstSel}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [firstSel, rowsData]);

  const handleRowClick = (uid: number, e: React.MouseEvent) => {
    if (!onSelect) return;
    if (e.shiftKey && anchor != null) {
      const a = orderedUids.indexOf(anchor);
      const b = orderedUids.indexOf(uid);
      if (a >= 0 && b >= 0) {
        const [lo, hi] = a < b ? [a, b] : [b, a];
        onSelect(orderedUids.slice(lo, hi + 1));
        return;
      }
    }
    if (e.ctrlKey || e.metaKey) {
      const next = new Set(sel);
      next.has(uid) ? next.delete(uid) : next.add(uid);
      onSelect([...next]);
      setAnchor(uid);
      return;
    }
    onSelect([uid]);
    setAnchor(uid);
  };

  const pad = (cells: Cell[], n: number) => Array.from({ length: n }, (_, i) => cells[i]);

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column", userSelect: "none" }}>
      {/* toolbar */}
      <div style={{ display: "flex", alignItems: "center", gap: 6, padding: "4px 6px", borderBottom: "1px solid hsl(var(--border))", flexShrink: 0 }}>
        {canGoUp && (
          <button title="Up one level" onClick={onUp} style={iconBtn}>
            <FolderUp size={14} />
          </button>
        )}
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="search…"
          style={{ flex: 1, minWidth: 0, padding: "3px 6px", fontSize: 11, color: "hsl(var(--foreground))", background: "hsl(var(--background))", border: "1px solid hsl(var(--border))", borderRadius: 3 }}
        />
        <button title="Sort direction" onClick={() => setDir((d) => (d === 1 ? -1 : 1))} style={iconBtn}>
          {dir === 1 ? <ArrowDown size={14} /> : <ArrowUp size={14} />}
        </button>
        <button title={showHidden ? "Hide hidden props" : "Show hidden props"} onClick={() => setShowHidden((h) => !h)} style={iconBtn}>
          {showHidden ? <Eye size={14} /> : <EyeOff size={14} />}
        </button>
      </div>

      {/* table */}
      <div ref={scrollRef} style={{ flex: 1, minHeight: 0, overflow: "auto" }}>
        <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
          <thead>
            <tr style={{ position: "sticky", top: 0, background: "hsl(var(--card))", color: "hsl(var(--muted-foreground))", zIndex: 1 }}>
              <th style={th}>Name</th>
              {maxIn > 0 && <th style={{ ...th, borderLeft: groupBorder }} colSpan={maxIn}>Inputs</th>}
              {maxOut > 0 && <th style={{ ...th, borderLeft: groupBorder }} colSpan={maxOut}>Outputs</th>}
              {maxCfg > 0 && <th style={{ ...th, borderLeft: groupBorder }} colSpan={maxCfg}>Config</th>}
            </tr>
          </thead>
          <tbody>
            {rowsData.map((r, ri) => (
              <tr
                key={r.c.uid}
                data-uid={r.c.uid}
                onClick={(e) => handleRowClick(r.c.uid, e)}
                onDoubleClick={() => onDrillIn?.(r.c.uid)}
                style={{
                  background: sel.has(r.c.uid) ? "hsl(var(--cool) / 0.18)" : "transparent",
                  cursor: onSelect ? "pointer" : "default",
                  // divider above the pinned selected-but-filtered orphans
                  borderTop: ri === orphanStart && orphanStart < rowsData.length ? "2px solid hsl(var(--border))" : undefined,
                }}
              >
                <td
                  style={{ ...td, color: "hsl(var(--foreground))", whiteSpace: "nowrap" }}
                  onContextMenu={(e) => {
                    if (!onNameContextMenu) return;
                    e.preventDefault();
                    onNameContextMenu(r.c.uid, e.clientX, e.clientY);
                  }}
                >
                  {r.c.name}
                </td>
                {(["i", "o", "g"] as const).flatMap((kind) => {
                  const cells = kind === "i" ? r.inputs : kind === "o" ? r.outputs : r.config;
                  const n = kind === "i" ? maxIn : kind === "o" ? maxOut : maxCfg;
                  return pad(cells, n).map((cell, i) => (
                    <ValueCell
                      key={`${kind}${i}`}
                      comp={r.c}
                      cell={cell}
                      groupStart={i === 0}
                      isEditing={!!cell && defaultEdit?.comp === r.c.uid && defaultEdit.uid === cell.uid}
                      onStartEdit={startEdit}
                      onCommitDefault={commitDefault}
                      onCancelEdit={cancelEdit}
                      onOpenOverride={openOverride}
                    />
                  ));
                })}
              </tr>
            ))}
            {rowsData.length === 0 && (
              <tr>
                <td colSpan={1 + maxIn + maxOut + maxCfg} style={{ ...td, color: "hsl(var(--muted-foreground))", textAlign: "center", padding: 16 }}>
                  no components in this folder
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {override && (
        <OverrideEditor
          rect={override.rect}
          cellLabel={override.cell.label}
          initial={useValues.getState().values.get(override.cell.uid)}
          dataType={propertyDataType.get(override.cell.uid) ?? DATATYPE_NUMBER}
          facet={withChoices(facets.get(override.comp)?.get(override.cell.uid), components.get(override.comp)?.type, override.cell.name)}
          overridden={((useStatusFlags.getState().flags.get(override.cell.uid) ?? 0) & STATUS_OVERRIDDEN) !== 0}
          onOverride={(v, dur) => {
            onSetOverride?.(override.comp, override.cell.name, v, dur);
            setOverride(null);
          }}
          onClear={() => {
            onClearOverride?.(override.comp, override.cell.name);
            setOverride(null);
          }}
          onClose={() => setOverride(null)}
        />
      )}
    </div>
  );
}

const groupBorder = "1px solid hsl(var(--border))";
const th: React.CSSProperties = { textAlign: "left", fontWeight: 500, padding: "5px 8px", borderBottom: "1px solid hsl(var(--border))" };
const td: React.CSSProperties = { padding: "4px 8px", borderBottom: "1px solid hsl(var(--secondary))", verticalAlign: "top" };
const iconBtn: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  width: 24,
  height: 24,
  border: "none",
  borderRadius: 3,
  background: "transparent",
  color: "hsl(var(--muted-foreground))",
  cursor: "pointer",
  flexShrink: 0,
};
