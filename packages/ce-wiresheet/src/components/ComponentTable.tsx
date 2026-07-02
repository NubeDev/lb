import { useEffect, useMemo, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { useStructural, useValues, useStatusFlags, propertyDataType } from "../lib/store";
import {
  ROLE_NORMAL,
  ROLE_STATUS,
  CATEGORY_INPUT,
  CATEGORY_OUTPUT,
  CATEGORY_CONFIG,
  STATUS_OVERRIDDEN,
  DATATYPE_BOOL,
  DATATYPE_NUMBER,
} from "../lib/engine-types";
import type { Component, FlexValue } from "../lib/engine-types";
import type { DecodedValue } from "../lib/wire";
import { facetFor, rawFacet, exposedPorts, type PropFacet, type ComponentFacet } from "../lib/facet";
import { fmtValueFacet, inferDataType } from "../lib/format";
import { Layers, ArrowUp, ArrowDown, FolderUp, Link2, CornerDownRight, EyeOff } from "lucide-react";

// Table view of the CURRENT folder's components. Each component is a row; its
// props align into per-slot columns under Inputs / Outputs / Config category
// headers (no per-prop headers — each cell self-labels with the facet label).
// Input cells are editable (commit sets an override); outputs are read-only.
// Double-click a row centers the wiresheet on that component.

// Same formatting as the wiresheet (shared lib/format). dataType is inferred
// when the prop isn't in the schema's streamable table.
const fmtCell = (v: DecodedValue | undefined, uid: number, facet: PropFacet | undefined): string =>
  fmtValueFacet(v, propertyDataType.get(uid) ?? inferDataType(v), facet);

interface Cell {
  uid: number;
  name: string;
  label: string;
  category: number;
  exposed?: boolean; // a child prop projected onto this (folder) component
  hidden?: boolean; // facet-hidden (only shown when "show hidden" is on)
}
interface RowCells {
  inputs: Cell[];
  outputs: Cell[];
  config: Cell[];
}

export function ComponentTable({
  currentParentUid,
  selectedUids,
  onSelect,
  onDrillIn,
  onNameContextMenu,
  onRowsChange,
  onSetOverride,
  onSetDefault,
  onClearOverride,
  canGoUp,
  onUp,
}: {
  currentParentUid: number;
  selectedUids: number[];
  onSelect: (uids: number[]) => void; // replaces the selection with these uids
  onDrillIn: (uid: number) => void;
  onNameContextMenu: (uid: number, x: number, y: number) => void;
  onRowsChange: (uids: number[]) => void;
  onSetOverride: (componentUid: number, property: string, value: FlexValue, duration: number) => void;
  onSetDefault: (componentUid: number, property: string, value: FlexValue) => void;
  onClearOverride: (componentUid: number, property: string) => void;
  canGoUp: boolean;
  onUp: () => void;
}) {
  const [showHidden, setShowHidden] = useState(false);
  const [query, setQuery] = useState("");
  const [dir, setDir] = useState<1 | -1>(1);
  // Click = edit the stored default inline; right-click = override popover.
  const [defaultEdit, setDefaultEdit] = useState<{ comp: number; uid: number } | null>(null);
  const [override, setOverride] = useState<{ comp: number; cell: Cell; rect: DOMRect } | null>(null);
  const [anchor, setAnchor] = useState<number | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  const components = useStructural((s) => s.components);
  const allRows = useMemo(
    () => Array.from(components.values()).filter((c) => c.parent === currentParentUid),
    [components, currentParentUid],
  );

  // Prop uids that are an endpoint of an edge → flagged as "linked" (wired).
  // Sourced from the store's linkedProps, which includes cross-folder edges.
  const linkedProps = useStructural((s) => s.linkedProps);

  const facets = useMemo(
    () =>
      new Map<number, ComponentFacet>(allRows.map((c) => [c.uid, facetFor(c.uid, rawFacet(c.properties))])),
    [allRows],
  );

  const cellsFor = (c: Component): RowCells => {
    const facet = facets.get(c.uid);
    const buckets: Record<number, (Cell & { order: number })[]> = {
      [CATEGORY_INPUT]: [],
      [CATEGORY_OUTPUT]: [],
      [CATEGORY_CONFIG]: [],
    };
    for (const [name, p] of Object.entries(c.properties)) {
      if ((p.systemRole ?? ROLE_NORMAL) !== ROLE_NORMAL) continue;
      const f = facet?.get(p.uid);
      // A wired (linked) prop can't be hidden — always show it.
      const hidden = !!f?.hidden && !linkedProps.has(p.uid);
      if (!showHidden && hidden) continue;
      const b = buckets[p.category];
      if (b)
        b.push({ uid: p.uid, name, label: f?.label || name, category: p.category, order: f?.order ?? 1e9, hidden });
    }
    // Exposed ports: child props this (folder) component projects as its own
    // input/output ports — read-only, value streams via the prop subscription.
    if (facet) {
      for (const ep of exposedPorts(facet)) {
        const hidden = !!ep.facet.hidden;
        if (!showHidden && hidden) continue;
        const cat = ep.side === "input" ? CATEGORY_INPUT : CATEGORY_OUTPUT;
        const label = ep.facet.label || `#${ep.childUid}`;
        buckets[cat].push({
          uid: ep.childUid,
          name: label,
          label,
          category: cat,
          order: ep.facet.order ?? 1e9,
          exposed: true,
          hidden,
        });
      }
    }
    const sortB = (arr: (Cell & { order: number })[]) =>
      arr.sort((a, b) => a.order - b.order || a.name.localeCompare(b.name));
    return {
      inputs: sortB(buckets[CATEGORY_INPUT]),
      outputs: sortB(buckets[CATEGORY_OUTPUT]),
      config: sortB(buckets[CATEGORY_CONFIG]),
    };
  };

  const { maxIn, maxOut, maxCfg } = useMemo(() => {
    let i = 0;
    let o = 0;
    let g = 0;
    for (const c of allRows) {
      const r = cellsFor(c);
      i = Math.max(i, r.inputs.length);
      o = Math.max(o, r.outputs.length);
      g = Math.max(g, r.config.length);
    }
    return { maxIn: i, maxOut: o, maxCfg: g };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [allRows, showHidden, facets, linkedProps]);

  const q = query.trim().toLowerCase();
  const matches = (c: Component) =>
    !q || (c.name || c.type).toLowerCase().includes(q) || c.type.toLowerCase().includes(q);

  const rows = useMemo(
    () =>
      allRows.filter(matches).sort((a, b) => (a.name || a.type).localeCompare(b.name || b.type) * dir),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [allRows, q, dir],
  );

  const watchUids = useMemo(() => {
    const set = new Set<number>();
    for (const c of allRows) {
      for (const p of Object.values(c.properties))
        if ((p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL || p.systemRole === ROLE_STATUS) set.add(p.uid);
      // Exposed-port child prop values (streamed at property level by the editor).
      for (const ep of exposedPorts(facets.get(c.uid) ?? new Map())) set.add(ep.childUid);
    }
    return [...set];
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [allRows, facets]);
  const values = useValues(
    useShallow((s) => {
      const out: Record<number, DecodedValue | undefined> = {};
      for (const uid of watchUids) out[uid] = s.values.get(uid);
      return out;
    }),
  );
  const flags = useStatusFlags(
    useShallow((s) => {
      const out: Record<number, number> = {};
      for (const uid of watchUids) out[uid] = s.flags.get(uid) ?? 0;
      return out;
    }),
  );

  useEffect(() => {
    onRowsChange(allRows.map((c) => c.uid));
    return () => onRowsChange([]);
  }, [allRows, onRowsChange]);

  const firstSel = selectedUids[0];
  useEffect(() => {
    if (firstSel == null) return;
    scrollRef.current
      ?.querySelector<HTMLElement>(`[data-uid="${firstSel}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [firstSel, rows]);

  const sel = new Set(selectedUids);
  const orphans = q ? allRows.filter((c) => sel.has(c.uid) && !matches(c)) : [];
  const totalCols = 1 + maxIn + maxOut + maxCfg;

  // Displayed row order (incl. pinned orphans) — used for shift-range selection.
  const orderedUids = useMemo(() => [...rows, ...orphans].map((c) => c.uid), [rows, orphans]);

  // Selection gestures: plain = single; ctrl/cmd = toggle; shift = range from the
  // anchor (last plain/toggle click) to the clicked row, inclusive.
  const handleRowClick = (uid: number, e: React.MouseEvent) => {
    if (e.shiftKey && anchor != null) {
      const a = orderedUids.indexOf(anchor);
      const b = orderedUids.indexOf(uid);
      if (a >= 0 && b >= 0) {
        const [lo, hi] = a <= b ? [a, b] : [b, a];
        onSelect(orderedUids.slice(lo, hi + 1));
        return;
      }
    }
    if (e.metaKey || e.ctrlKey) {
      const next = new Set(selectedUids);
      if (next.has(uid)) next.delete(uid);
      else next.add(uid);
      onSelect([...next]);
      setAnchor(uid);
      return;
    }
    onSelect([uid]);
    setAnchor(uid);
  };

  const valueCell = (cell: Cell | undefined, c: Component, groupStart: boolean, slot: string) => {
    const overridden = cell != null && (flags[cell.uid] & STATUS_OVERRIDDEN) !== 0;
    // Exposed ports are read-only here (override/default go through the child's
    // own component, not the folder); outputs are computed.
    const editableDefault = cell != null && cell.category !== CATEGORY_OUTPUT && !cell.exposed;
    const isEditingDefault = cell != null && defaultEdit?.comp === c.uid && defaultEdit.uid === cell.uid;
    return (
      <td
        key={slot}
        onContextMenu={(e) => {
          if (cell && !cell.exposed) {
            e.preventDefault();
            e.stopPropagation();
            setOverride({ comp: c.uid, cell, rect: e.currentTarget.getBoundingClientRect() });
          }
        }}
        style={{
          padding: "4px 8px",
          whiteSpace: "nowrap",
          borderLeft: groupStart ? "1px solid hsl(var(--border))" : undefined,
          fontFamily: "var(--font-mono)",
        }}
      >
        {cell && (
          <span style={{ display: "inline-flex", gap: 5, alignItems: "baseline" }}>
            {cell.hidden && (
              <EyeOff size={11} color="hsl(var(--muted-foreground))" style={{ alignSelf: "center" }} aria-label="hidden" />
            )}
            {cell.exposed ? (
              <CornerDownRight size={11} color="hsl(var(--muted-foreground))" style={{ alignSelf: "center" }} aria-label="exposed" />
            ) : (
              linkedProps.has(cell.uid) && (
                <Link2 size={11} color="hsl(var(--cool))" style={{ alignSelf: "center" }} aria-label="linked" />
              )
            )}
            <span style={{ color: "hsl(var(--muted-foreground))" }}>{cell.label}</span>
            {isEditingDefault ? (
              <DefaultEditor
                initial={values[cell.uid]}
                dataType={propertyDataType.get(cell.uid) ?? DATATYPE_NUMBER}
                facet={facets.get(c.uid)?.get(cell.uid)}
                onCommit={(v) => {
                  onSetDefault(c.uid, cell.name, v);
                  setDefaultEdit(null);
                }}
                onCancel={() => setDefaultEdit(null)}
              />
            ) : (
              <span
                onClick={
                  editableDefault
                    ? (e) => {
                        e.stopPropagation();
                        setDefaultEdit({ comp: c.uid, uid: cell.uid });
                      }
                    : undefined
                }
                title={
                  editableDefault
                    ? "click to edit default · right-click to override"
                    : "right-click to override"
                }
                style={{
                  color: cell.category === CATEGORY_INPUT ? "hsl(var(--foreground))" : "hsl(var(--foreground))",
                  cursor: editableDefault ? "pointer" : "default",
                  borderBottom: editableDefault ? "1px dotted hsl(var(--input))" : undefined,
                }}
              >
                {fmtCell(values[cell.uid], cell.uid, facets.get(c.uid)?.get(cell.uid))}
              </span>
            )}
            {overridden && <OvrBadge />}
          </span>
        )}
      </td>
    );
  };

  const renderRow = (c: Component) => {
    const r = cellsFor(c);
    const isFolder = (c.childrenCount ?? 0) > 0;
    return (
      <tr
        key={c.uid}
        data-uid={c.uid}
        // Plain = select; Ctrl/Cmd = toggle; Shift = range from the anchor;
        // double-click = go inside; Space (editor level) focuses the canvas.
        onClick={(e) => handleRowClick(c.uid, e)}
        onDoubleClick={() => onDrillIn(c.uid)}
        title="double-click to go inside · ctrl-click to toggle · shift-click to range-select · space to focus on canvas"
        style={{
          cursor: "pointer",
          background: sel.has(c.uid) ? "hsl(var(--cool) / 0.18)" : "transparent",
          borderBottom: "1px solid hsl(var(--secondary))",
        }}
      >
        <td
          onContextMenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            onNameContextMenu(c.uid, e.clientX, e.clientY);
          }}
          style={{ padding: "4px 10px", fontFamily: "var(--font-mono)" }}
        >
          <span style={{ display: "flex", alignItems: "center", gap: 4, fontWeight: 600 }}>
            {isFolder && <Layers size={12} color="hsl(var(--cool))" />}
            <span style={{ color: "hsl(var(--foreground))" }}>{c.name || c.type}</span>
          </span>
        </td>
        {Array.from({ length: maxIn }, (_, i) => valueCell(r.inputs[i], c, i === 0, `i${i}`))}
        {Array.from({ length: maxOut }, (_, i) => valueCell(r.outputs[i], c, i === 0, `o${i}`))}
        {Array.from({ length: maxCfg }, (_, i) => valueCell(r.config[i], c, i === 0, `g${i}`))}
      </tr>
    );
  };

  return (
    <div
      style={{
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: "hsl(var(--background))",
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
        fontSize: 12,
        minWidth: 0,
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          padding: "6px 10px",
          borderBottom: "1px solid hsl(var(--border))",
          flexShrink: 0,
        }}
      >
        <button
          onClick={onUp}
          disabled={!canGoUp}
          title="Up to parent folder"
          style={{
            display: "flex",
            alignItems: "center",
            background: "transparent",
            border: "1px solid hsl(var(--border))",
            borderRadius: 3,
            color: canGoUp ? "hsl(var(--foreground))" : "hsl(var(--input))",
            cursor: canGoUp ? "pointer" : "default",
            padding: "3px 6px",
            flexShrink: 0,
          }}
        >
          <FolderUp size={14} />
        </button>
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="search components…"
          spellCheck={false}
          style={{
            flex: 1,
            minWidth: 0,
            background: "hsl(var(--secondary))",
            color: "hsl(var(--foreground))",
            border: "1px solid hsl(var(--border))",
            borderRadius: 3,
            padding: "4px 7px",
            fontSize: 11,
            fontFamily: "var(--font-mono)",
            outline: "none",
          }}
        />
        <button
          onClick={() => setDir((d) => (d === 1 ? -1 : 1))}
          title={`Sort by name ${dir === 1 ? "ascending" : "descending"}`}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 3,
            background: "transparent",
            border: "1px solid hsl(var(--border))",
            borderRadius: 3,
            color: "hsl(var(--muted-foreground))",
            cursor: "pointer",
            padding: "3px 6px",
            fontSize: 11,
            flexShrink: 0,
          }}
        >
          name {dir === 1 ? <ArrowUp size={11} /> : <ArrowDown size={11} />}
        </button>
        <label style={{ display: "flex", alignItems: "center", gap: 4, color: "hsl(var(--muted-foreground))", flexShrink: 0 }}>
          <input type="checkbox" checked={showHidden} onChange={(e) => setShowHidden(e.target.checked)} />
          hidden
        </label>
      </div>

      <div ref={scrollRef} style={{ overflow: "auto", flex: 1 }}>
        {rows.length === 0 && orphans.length === 0 ? (
          <div style={{ padding: 12, color: "hsl(var(--muted-foreground))" }}>
            {allRows.length === 0 ? "no components in this folder" : "no matches"}
          </div>
        ) : (
          <table style={{ borderCollapse: "collapse", whiteSpace: "nowrap", userSelect: "none" }}>
            <thead>
              <tr style={{ position: "sticky", top: 0, background: "hsl(var(--card))", zIndex: 1 }}>
                <GroupTh />
                {maxIn > 0 && <GroupTh span={maxIn}>Inputs</GroupTh>}
                {maxOut > 0 && <GroupTh span={maxOut}>Outputs</GroupTh>}
                {maxCfg > 0 && <GroupTh span={maxCfg}>Config</GroupTh>}
              </tr>
            </thead>
            <tbody>
              {rows.map(renderRow)}
              {orphans.length > 0 && (
                <>
                  <tr>
                    <td
                      colSpan={totalCols}
                      style={{
                        padding: "5px 10px",
                        background: "hsl(var(--crit) / 0.2)",
                        borderTop: "1px solid hsl(var(--crit) / 0.2)",
                        borderBottom: "1px solid hsl(var(--crit) / 0.2)",
                        color: "hsl(var(--amber))",
                        fontSize: 10,
                        textTransform: "uppercase",
                        letterSpacing: 0.4,
                      }}
                    >
                      selected · filtered out ({orphans.length})
                    </td>
                  </tr>
                  {orphans.map(renderRow)}
                </>
              )}
            </tbody>
          </table>
        )}
      </div>

      {override && (
        <OverrideEditor
          rect={override.rect}
          cell={override.cell}
          initial={values[override.cell.uid]}
          dataType={propertyDataType.get(override.cell.uid) ?? DATATYPE_NUMBER}
          facet={facets.get(override.comp)?.get(override.cell.uid)}
          overridden={(flags[override.cell.uid] & STATUS_OVERRIDDEN) !== 0}
          onOverride={(v, duration) => {
            onSetOverride(override.comp, override.cell.name, v, duration);
            setOverride(null);
          }}
          onClear={() => {
            onClearOverride(override.comp, override.cell.name);
            setOverride(null);
          }}
          onClose={() => setOverride(null)}
        />
      )}
    </div>
  );
}

// Shared value coercion for the editors.
const codeOf = (v: DecodedValue | undefined) =>
  v === true ? 1 : v === false ? 0 : typeof v === "number" ? v : Number(v);
const initialStr = (
  v: DecodedValue | undefined,
  dataType: number,
  facet: PropFacet | undefined,
) =>
  facet?.aliases?.length || dataType === DATATYPE_BOOL
    ? String(codeOf(v))
    : v == null
      ? ""
      : String(v);
const coerceValue = (raw: string, dataType: number, facet: PropFacet | undefined): FlexValue => {
  if (facet?.aliases?.length) {
    const code = Number(raw);
    return dataType === DATATYPE_BOOL ? code === 1 : code;
  }
  if (dataType === DATATYPE_BOOL) return raw === "1" || raw === "true";
  if (dataType === DATATYPE_NUMBER) return Number(raw);
  return raw;
};
const editField = {
  background: "hsl(var(--background))",
  color: "hsl(var(--foreground))",
  border: "1px solid hsl(var(--cool))",
  borderRadius: 2,
  padding: "1px 4px",
  fontSize: 12,
  fontFamily: "var(--font-mono)",
  outline: "none",
} as const;

// Inline editor for the stored DEFAULT value (left-click). Dropdown for aliased /
// boolean props, else a text/number field. Enter / blur commits, Esc cancels.
function DefaultEditor({
  initial,
  dataType,
  facet,
  onCommit,
  onCancel,
}: {
  initial: DecodedValue | undefined;
  dataType: number;
  facet: PropFacet | undefined;
  onCommit: (v: FlexValue) => void;
  onCancel: () => void;
}) {
  const aliases = facet?.aliases;
  const [text, setText] = useState(initialStr(initial, dataType, facet));
  const ref = useRef<HTMLInputElement | HTMLSelectElement>(null);
  useEffect(() => {
    ref.current?.focus();
    if (ref.current instanceof HTMLInputElement) ref.current.select();
  }, []);
  const onKey = (e: React.KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") onCommit(coerceValue(text, dataType, facet));
    else if (e.key === "Escape") onCancel();
  };
  if (aliases?.length || dataType === DATATYPE_BOOL) {
    const opts = aliases?.length
      ? aliases.map((a) => ({ v: String(a.code), label: a.label }))
      : [
          { v: "0", label: "false" },
          { v: "1", label: "true" },
        ];
    return (
      <select
        ref={ref as React.RefObject<HTMLSelectElement>}
        value={text}
        onChange={(e) => onCommit(coerceValue(e.target.value, dataType, facet))}
        onKeyDown={onKey}
        onBlur={onCancel}
        style={editField}
      >
        {opts.map((o) => (
          <option key={o.v} value={o.v}>
            {o.label}
          </option>
        ))}
      </select>
    );
  }
  return (
    <input
      ref={ref as React.RefObject<HTMLInputElement>}
      value={text}
      onChange={(e) => setText(e.target.value)}
      onKeyDown={onKey}
      onBlur={() => onCommit(coerceValue(text, dataType, facet))}
      inputMode={dataType === DATATYPE_NUMBER ? "decimal" : "text"}
      style={{ ...editField, width: 70 }}
    />
  );
}

// Override popover (right-click) — value + duration, set / clear. Matches the
// wiresheet's override flow.
function OverrideEditor({
  rect,
  cell,
  initial,
  dataType,
  facet,
  overridden,
  onOverride,
  onClear,
  onClose,
}: {
  rect: DOMRect;
  cell: Cell;
  initial: DecodedValue | undefined;
  dataType: number;
  facet: PropFacet | undefined;
  overridden: boolean;
  onOverride: (v: FlexValue, duration: number) => void;
  onClear: () => void;
  onClose: () => void;
}) {
  const aliases = facet?.aliases;
  const [text, setText] = useState(initialStr(initial, dataType, facet));
  const [duration, setDuration] = useState("60"); // matches standard override menu
  const ref = useRef<HTMLInputElement | HTMLSelectElement>(null);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    ref.current?.focus();
    if (ref.current instanceof HTMLInputElement) ref.current.select();
  }, []);
  useEffect(() => {
    const dismiss = (e: PointerEvent) => {
      if (!rootRef.current?.contains(e.target as Node)) onClose();
    };
    document.addEventListener("pointerdown", dismiss, true);
    return () => document.removeEventListener("pointerdown", dismiss, true);
  }, [onClose]);

  const apply = () => onOverride(coerceValue(text, dataType, facet), Number(duration) || 0);

  const field = { ...editField, border: "1px solid hsl(var(--border))", borderRadius: 3, padding: "3px 6px" };
  const btn = (accent?: boolean) =>
    ({
      background: accent ? "hsl(var(--cool) / 0.18)" : "transparent",
      color: accent ? "hsl(var(--cool))" : "hsl(var(--muted-foreground))",
      border: `1px solid ${accent ? "hsl(var(--cool))" : "hsl(var(--border))"}`,
      borderRadius: 3,
      padding: "3px 8px",
      cursor: "pointer",
      fontSize: 11,
    }) as const;

  const left = Math.min(rect.left, window.innerWidth - 220);
  const top = Math.min(rect.bottom + 4, window.innerHeight - 160);

  return (
    <div
      ref={rootRef}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Escape") onClose();
        else if (e.key === "Enter" && !(e.target instanceof HTMLSelectElement)) apply();
      }}
      style={{
        position: "fixed",
        left,
        top,
        zIndex: 200,
        width: 200,
        background: "hsl(var(--card))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 6,
        boxShadow: "0 8px 24px rgba(0,0,0,0.6)",
        padding: 8,
        display: "flex",
        flexDirection: "column",
        gap: 6,
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
        fontSize: 12,
      }}
    >
      <div style={{ color: "hsl(var(--cool))", fontFamily: "var(--font-mono)", fontSize: 11 }}>
        Override {cell.label}
      </div>

      {aliases?.length || dataType === DATATYPE_BOOL ? (
        <select
          ref={ref as React.RefObject<HTMLSelectElement>}
          value={text}
          onChange={(e) => setText(e.target.value)}
          style={field}
        >
          {(aliases?.length
            ? aliases.map((a) => ({ v: String(a.code), label: a.label }))
            : [
                { v: "0", label: "false" },
                { v: "1", label: "true" },
              ]
          ).map((o) => (
            <option key={o.v} value={o.v}>
              {o.label}
            </option>
          ))}
        </select>
      ) : (
        <input
          ref={ref as React.RefObject<HTMLInputElement>}
          value={text}
          onChange={(e) => setText(e.target.value)}
          inputMode={dataType === DATATYPE_NUMBER ? "decimal" : "text"}
          placeholder={facet?.unit}
          style={field}
        />
      )}

      <label style={{ display: "flex", alignItems: "center", gap: 6, color: "hsl(var(--muted-foreground))", fontSize: 11 }}>
        duration
        <select value={duration} onChange={(e) => setDuration(e.target.value)} style={{ ...field, flex: 1 }}>
          <option value="10">10 sec</option>
          <option value="30">30 sec</option>
          <option value="60">1 min</option>
          <option value="300">5 min</option>
          <option value="1200">20 min</option>
          <option value="3600">1 hr</option>
          <option value="7200">2 hr</option>
          <option value="86400">24 hr</option>
          <option value="0">permanent</option>
        </select>
      </label>

      <div style={{ display: "flex", gap: 6, marginTop: 2 }}>
        {overridden && (
          <button onClick={onClear} style={{ ...btn(), color: "hsl(var(--crit))" }} title="Clear override">
            clear
          </button>
        )}
        <button onClick={onClose} style={{ ...btn(), marginLeft: "auto" }}>
          cancel
        </button>
        <button onClick={apply} style={btn(true)}>
          set
        </button>
      </div>
    </div>
  );
}

// Same OVR badge the wiresheet node rows use.
function OvrBadge() {
  return (
    <span
      title="overridden"
      style={{
        fontSize: 9,
        padding: "0 4px",
        background: "hsl(var(--amber))",
        color: "hsl(var(--background))",
        borderRadius: 2,
        fontWeight: 600,
        fontFamily: "var(--font-mono)",
      }}
    >
      OVR
    </span>
  );
}

function GroupTh({ children, span }: { children?: React.ReactNode; span?: number }) {
  return (
    <th
      colSpan={span}
      style={{
        textAlign: "left",
        padding: "4px 10px",
        borderBottom: "1px solid hsl(var(--border))",
        borderLeft: children ? "1px solid hsl(var(--border))" : undefined,
        color: "hsl(var(--muted-foreground))",
        fontWeight: 600,
        fontSize: 10,
        textTransform: "uppercase",
        letterSpacing: 0.4,
        fontFamily: "var(--font-mono)",
      }}
    >
      {children}
    </th>
  );
}
