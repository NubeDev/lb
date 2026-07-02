import { createContext, memo, useContext, useEffect, useMemo, useRef, useState } from "react";
import { Zap, EyeOff, Layers, CornerDownRight, ChevronRight, ChevronDown } from "lucide-react";
import { createPortal } from "react-dom";
import { wiresheetPortalRoot } from "../lib/portal";
import { Handle, Position, useStore as useRfStore } from "@xyflow/react";
import { useShallow } from "zustand/react/shallow";
import {
  useValues,
  useSchemaVersion,
  useStructural,
  useStatusFlags,
  propertyDataType,
} from "../lib/store";
import { diagRecordRender } from "../lib/diagnostics";
import { usePresence, PRESENCE_PALETTE } from "../lib/presence";
import {
  STATUS_OVERRIDDEN,
  CATEGORY_INPUT,
  CATEGORY_OUTPUT,
  CATEGORY_CONFIG,
  DATATYPE_NUMBER,
  DATATYPE_BOOL,
  DATATYPE_STRING,
  ROLE_NORMAL,
  ROLE_STATUS,
  type Component,
  type Edge as EdgeT,
  type Property,
  type PropertyCategory,
  type PropertyDataType,
  type PropertySystemRole,
} from "../lib/engine-types";
import type { DecodedValue } from "../lib/wire";
import { facetFor, rawFacet, exposedPorts, FACET_PROP, type PropFacet, type ComponentFacet } from "../lib/facet";
import { withChoices } from "../lib/choices";
import { fmtValueFacet, inferDataType } from "../lib/format";
import { CopyButton } from "../ui/CopyButton";
import {
  buildConnectGroups,
  filterConnectGroups,
  takenInputUids,
  connectTier,
} from "../lib/connect";

// Editor-level capabilities the ConnectPicker needs for its "New" flow — the
// creatable component types and a "create one in the current folder" action.
// Supplied by CeEditor via context (crosses the picker's createPortal).
export interface CeWiresheetCtx {
  componentTypes: Array<{ name: string; type: string; group: string }>;
  createComponent: (
    type: string,
    opts?: { nearUid?: number; side?: "left" | "right" },
  ) => Promise<Component | null>;
  // Add an edge and update the view incrementally (append in-folder; reload only
  // for a cross-folder target that needs a ghost). Avoids a full reload per link.
  connectEdge: (payload: {
    sourceUid: number;
    sourcePropUid: number;
    targetUid: number;
    targetPropUid: number;
  }) => Promise<void>;
  // Expose a child's prop as a port on the current container (folder). Present
  // only when inside a container (not at root); parentName is that container's
  // display name for the menu label.
  parentName?: string;
  // Pin a child prop as a port on the current folder (the engine maintainer derives
  // side/owner/name; we just pin the prop uid).
  exposeProp?: (childPropUid: number) => void | Promise<void>;
  // Clear a manual pin on `folderUid` (the folder the port is drawn on).
  unexposeProp?: (folderUid: number, childPropUid: number) => void | Promise<void>;
  // Open the Details panel for any component (e.g. the off-canvas child behind an
  // exposed port, so its facet — the source of truth — can be edited there).
  openDetails?: (componentUid: number) => void;
  // Request a (debounced) scope reload. Used when a component's live __facets
  // stream changes its EXPOSED-PORT set (expose/unexpose from another session):
  // that alters port handles and thus edge routing, so ghosts/ports must be
  // rebuilt — a row re-derive isn't enough. Cosmetic facet edits don't call this.
  requestReload?: () => void;
}
export const CeWiresheetContext = createContext<CeWiresheetCtx | null>(null);

// Composite view of a property assembled from REST (structure + statusFlags
// snapshot) and WS schema (dataType). One row per non-CONFIG, non-system Property.
interface PropRow {
  uid: number;
  name: string;
  category: PropertyCategory;
  dataType: PropertyDataType;
  systemRole?: PropertySystemRole;
  facet?: PropFacet; // per-prop presentation metadata from __facets
  exposed?: boolean; // a child prop projected onto this (parent) as a port
  exposedComponent?: number; // for an exposed port, the child component that owns it
  facetPropUid?: number; // for an exposed port, the child's __facets prop uid (live)
}

export type FunctionBlockData = {
  componentUid: number;
  // Display name from REST (e.g. "add", "Heartbeat1"). Shown in the title bar; the
  // component TYPE is shown below it (smaller). Both come from REST since the WS
  // schema only carries `kind` (= type), not the instance name.
  name?: string;
  // True if this component has children — drives the "↵ enter" affordance and a small
  // badge in the title bar. Filled in by App.tsx from the REST `childrenCount` field.
  hasChildren?: boolean;
  childCount?: number;
  // True if this component's TYPE declares any actions (from /schema). Drives the
  // ⚡ marker in the bottom lip. Filled in by App.tsx from the action index.
  hasActions?: boolean;
  // Click-into handler. Provided by App.tsx so the block doesn't have to know about
  // routing/breadcrumb state.
  onEnter?: (uid: number) => void;
  // Node-level right-click handler. Provided by App.tsx so it can open a menu
  // that operates on the current multi-selection (reparent etc.). Property rows
  // intercept their own onContextMenu so this only fires on the node body
  // (title bar + spacing).
  onContextMenu?: (uid: number, x: number, y: number) => void;
} & Record<string, unknown>;

const COLOR_NUMBER = "hsl(var(--cool))";
const COLOR_BOOL = "hsl(var(--green))";
const COLOR_STRING = "hsl(var(--amber))";

// Two stacked text lines (12px name + 10px type) need ~32px with default line
// heights; the title bar pads 4px top + 4px bottom, so the content box must be
// at least 32px → 40px outer height. Less and the type label's descenders get
// clipped by overflow: hidden on the node root.
const TITLE_H = 40;
const ROW_H = 18;
export const NODE_W = 220;

// Height of the GhostNode (sub-node) that represents an off-canvas component
// endpoint of a cross-folder edge — exactly one property row so it lines up
// flush with the source/target prop on the visible component. Width is
// computed per-ghost from the content (see ghostWidthFor) instead of using a
// fixed value, so a short label like "root · out" doesn't render a half-empty
// box of padding.
export const GHOST_H = ROW_H;
export const GHOST_W_MIN = 56;
export const GHOST_W_MAX = 260;

// Estimate width needed for `<path> · <propName>` rendered in the same 10px
// monospace font + padding the ghost uses. Slight overshoot (6.2px/char)
// since glyph widths vary; ellipsis handles any remaining overflow.
export function ghostWidthFor(path: string, propName: string): number {
  const text = `${path || "root"} · ${propName}`;
  // 22px = horizontal padding (8 + 8) + handle marker (8) − a couple px the
  // marker overlaps the edge.
  const w = 22 + Math.ceil(text.length * 6.2);
  return Math.max(GHOST_W_MIN, Math.min(GHOST_W_MAX, w));
}

// Drop the leading "root/" (or bare "root") from a component path. Every path
// starts at root, so the prefix is noise — labels read more cleanly without
// it. Used both for ghost labels and the popover list rows so the same path
// formatting is applied everywhere a cross-folder location is shown.
export function stripRoot(path: string): string {
  if (path === "root" || path === "") return "root";
  if (path.startsWith("root/")) return path.slice(5);
  return path;
}

// Just the component's own name (the last path segment) — the compact label a
// collapsed ghost shows, so the pill hugs the prop row instead of spelling out
// the whole folder chain. The full path is still one hover (title) or click
// (popover) away.
export function lastSegment(path: string): string {
  const s = stripRoot(path);
  const i = s.lastIndexOf("/");
  return i >= 0 ? s.slice(i + 1) : s;
}

// Human-readable dataType label (tooltips only; never compared).
const DATATYPE_LABEL: Record<number, string> = {
  [DATATYPE_NUMBER]: "number",
  [DATATYPE_BOOL]: "bool",
  [DATATYPE_STRING]: "string",
};

// Below this zoom the graph is a far overview — individual values aren't
// legible, so nodes render in a cheap level-of-detail form: title + handles
// only, no property rows / value cells, and the value/status subscriptions go
// dormant (no per-frame re-renders). Cuts DOM + reconcile cost dramatically
// when the whole graph (100s of nodes) is on screen. Kept low (0.12) so full
// detail stays through normal working zooms and LOD only kicks in on a deep
// zoom-out.
const LOD_ZOOM = 0.12;

function colorForType(dt: PropertyDataType): string {
  if (dt === DATATYPE_BOOL) return COLOR_BOOL;
  if (dt === DATATYPE_STRING) return COLOR_STRING;
  return COLOR_NUMBER;
}

// A uid rendered as a click-to-copy chip (menus aren't selection-friendly — the
// app uses user-select:none and the menu dismisses on pointerdown).
export function CopyUid({ label, value }: { label: string; value: number }) {
  const [copied, setCopied] = useState(false);
  return (
    <span
      onClick={(e) => {
        e.stopPropagation();
        void navigator.clipboard?.writeText(String(value)).then(
          () => {
            setCopied(true);
            window.setTimeout(() => setCopied(false), 900);
          },
          () => {},
        );
      }}
      title="click to copy"
      style={{
        cursor: "pointer",
        textDecoration: "underline dotted",
        color: copied ? "hsl(var(--green))" : "inherit",
      }}
    >
      {label} {copied ? "copied" : value}
    </span>
  );
}

// Like CopyUid but copies an arbitrary string while showing a (possibly
// truncated) display label — used for the component path in the node menu.
export function CopyText({ display, value, title = "click to copy" }: { display: string; value: string; title?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <span
      onClick={(e) => {
        e.stopPropagation();
        void navigator.clipboard?.writeText(value).then(
          () => {
            setCopied(true);
            window.setTimeout(() => setCopied(false), 900);
          },
          () => {},
        );
      }}
      title={`${title} — ${value}`}
      style={{ cursor: "pointer", textDecoration: "underline dotted", color: copied ? "hsl(var(--green))" : "inherit" }}
    >
      {copied ? "copied" : display}
    </span>
  );
}

// Trim a component path to its trailing segments so it fits a small menu: keeps
// the node and as many parents as `maxChars` allows, prefixing "…/" when cut.
// Copies the FULL path regardless (see CopyText). e.g. root/a/b/c/d/leaf → …/c/d/leaf
export function ellipsizePath(path: string, maxChars = 38): string {
  const clean = stripRoot(path);
  if (clean.length <= maxChars) return clean;
  const segs = clean.split("/");
  let out = segs[segs.length - 1];
  for (let i = segs.length - 2; i >= 0; i--) {
    const next = `${segs[i]}/${out}`;
    if (next.length + 2 > maxChars) break; // +2 for the "…/" prefix
    out = next;
  }
  return `…/${out}`;
}

// Right-click menu for a property row. Set / clear an override, or initiate an
// edge from this property via "Connect to…".
function PropertyContextMenu({
  x,
  y,
  propName,
  propUid,
  category,
  dataType,
  currentValue,
  overridden,
  exposed,
  portOwner,
  componentUid,
  onClose,
}: {
  x: number;
  y: number;
  propName: string;
  propUid: number;
  category: PropertyCategory;
  dataType: PropertyDataType;
  currentValue: DecodedValue | undefined;
  overridden: boolean;
  exposed?: boolean;
  portOwner?: number;
  componentUid: number;
  onClose: () => void;
}) {
  const [promptOpen, setPromptOpen] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [draft, setDraft] = useState<string>(
    currentValue == null ? "" : typeof currentValue === "string" ? currentValue : String(currentValue),
  );
  // Override duration in seconds. 0 = permanent (until cleared). Default to 1 minute
  // — a reasonable "I want to nudge this for a moment" length.
  const [durationSec, setDurationSec] = useState<number>(60);
  const ctx = useContext(CeWiresheetContext);

  useEffect(() => {
    const dismiss = (e: Event) => {
      const el = e.target as Element | null;
      // The picker carries its own data-ce-menu so its clicks are also
      // tolerated here — clicking inside it should NOT dismiss the menu.
      if (el && el.closest("[data-ce-menu]")) return;
      onClose();
    };
    // Capture-phase pointerdown: React Flow's pane stopImmediatePropagation's on
    // press, so a bubble-phase document mousedown never sees clicks on the canvas
    // (that's why click-away wasn't closing the menu). Capture fires first.
    document.addEventListener("pointerdown", dismiss, true);
    document.addEventListener("contextmenu", dismiss, true);
    return () => {
      document.removeEventListener("pointerdown", dismiss, true);
      document.removeEventListener("contextmenu", dismiss, true);
    };
  }, [onClose]);
  // Only edges between inputs and outputs make sense; config props don't
  // participate in dataflow edges, so "Connect to…" is hidden for them.
  const canConnect = category === CATEGORY_INPUT || category === CATEGORY_OUTPUT;
  // All normal properties can be overridden — including outputs, where the
  // override freezes the engine-computed value via PATCH /overrides. (The
  // inline click-to-edit on the row is still input/config only, since that
  // path PATCHes /nodes which wouldn't take on outputs.)
  // Exposed ports can't be overridden from here — override is name-based and the
  // row shows the port's label, not the child's real prop name (and the engine has
  // no prop-uid override yet). Override the real value inside the child component.
  const overridable =
    !exposed &&
    (category === CATEGORY_INPUT || category === CATEGORY_CONFIG || category === CATEGORY_OUTPUT);

  const parse = (raw: string): string | number | boolean | null => {
    const t = raw.trim();
    if (t === "") return null;
    if (dataType === DATATYPE_BOOL) {
      const lower = t.toLowerCase();
      return lower === "true" || lower === "1" || lower === "yes";
    }
    if (dataType === DATATYPE_STRING) return t;
    const n = Number(t);
    return Number.isFinite(n) ? n : null;
  };

  // Optimistic update — flip the property's status bits locally BEFORE the
  // network call so the OVR badge / amber tint appears the moment the user
  // clicks. The real value/status arrives via the WS binary frame within a few
  // ms; without this, the visual lags the click by the full HTTP round trip.
  // The STATUS section in the next frame will overwrite our optimistic value
  // with the authoritative one.
  const optimisticSetBit = async (uid: number, bit: number, on: boolean) => {
    const { useStatusFlags } = await import("../lib/store");
    const s = useStatusFlags.getState();
    const cur = s.flags.get(uid) ?? 0;
    const next = on ? cur | bit : cur & ~bit;
    s.applyStatus([uid], [next]);
  };

  const setOverride = async () => {
    const parsed = parse(draft);
    if (parsed == null) {
      onClose();
      return;
    }
    onClose();
    const { useStructural } = await import("../lib/store");
    const cur = useStructural.getState().components.get(componentUid);
    const uid = cur?.properties[propName]?.uid;
    if (uid != null) await optimisticSetBit(uid, STATUS_OVERRIDDEN, true);
    try {
      const { patchOverrides } = await import("../lib/rest");
      const updated = await patchOverrides(componentUid, {
        setOverrides: [
          { property: propName, value: parsed, duration: durationSec },
        ],
      });
      useStructural.getState().upsertComponent(updated);
    } catch (e) {
      console.error("set override failed:", (e as Error).message);
      // Roll the optimistic flip back. The next WS frame's STATUS section will
      // reconcile authoritatively anyway, but this is faster.
      if (uid != null) await optimisticSetBit(uid, STATUS_OVERRIDDEN, false);
    }
  };

  const clearOverride = async () => {
    onClose();
    const { useStructural } = await import("../lib/store");
    const cur = useStructural.getState().components.get(componentUid);
    const uid = cur?.properties[propName]?.uid;
    if (uid != null) await optimisticSetBit(uid, STATUS_OVERRIDDEN, false);
    try {
      const { patchOverrides } = await import("../lib/rest");
      const updated = await patchOverrides(componentUid, { clearOverrides: [propName] });
      useStructural.getState().upsertComponent(updated);
    } catch (e) {
      console.error("clear override failed:", (e as Error).message);
      if (uid != null) await optimisticSetBit(uid, STATUS_OVERRIDDEN, true);
    }
  };

  return createPortal(
    <div
      data-ce-menu
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        left: x,
        top: y,
        zIndex: 100,
        background: "hsl(var(--card))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 4,
        padding: 4,
        minWidth: 180,
        boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
        fontSize: 11,
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
      }}
    >
      <div
        style={{ padding: "4px 8px", color: "hsl(var(--muted-foreground))", borderBottom: "1px solid hsl(var(--border))", marginBottom: 4 }}
      >
        {propName} <span style={{ color: "hsl(var(--muted-foreground))" }}>· {dataType}</span>
        <div
          style={{
            fontSize: 9,
            color: "hsl(var(--muted-foreground))",
            fontFamily: "var(--font-mono)",
            marginTop: 2,
          }}
        >
          <CopyUid label="prop" value={propUid} /> · <CopyUid label="comp" value={componentUid} />
        </div>
      </div>
      {promptOpen ? (
        <div style={{ padding: "4px 6px", display: "flex", flexDirection: "column", gap: 4 }}>
          {dataType === DATATYPE_BOOL ? (
            <select
              autoFocus
              className="nodrag"
              value={draft || "true"}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") setOverride();
                else if (e.key === "Escape") onClose();
                e.stopPropagation();
              }}
              style={overrideInputStyle}
            >
              <option value="true">true</option>
              <option value="false">false</option>
            </select>
          ) : (
            <input
              autoFocus
              className="nodrag"
              type={dataType === DATATYPE_NUMBER ? "number" : "text"}
              inputMode={dataType === DATATYPE_NUMBER ? "decimal" : undefined}
              step={dataType === DATATYPE_NUMBER ? "any" : undefined}
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") setOverride();
                else if (e.key === "Escape") onClose();
                e.stopPropagation();
              }}
              style={overrideInputStyle}
              placeholder="override value…"
            />
          )}
          <label style={{ display: "flex", alignItems: "center", gap: 4, color: "hsl(var(--muted-foreground))", fontSize: 10 }}>
            <span style={{ flex: 1 }}>duration</span>
            <select
              className="nodrag"
              value={durationSec}
              onChange={(e) => setDurationSec(Number(e.target.value))}
              onClick={(e) => e.stopPropagation()}
              style={{
                background: "hsl(var(--background))",
                color: "hsl(var(--foreground))",
                border: "1px solid hsl(var(--border))",
                borderRadius: 2,
                padding: "2px 4px",
                fontSize: 11,
                fontFamily: "inherit",
              }}
            >
              <option value={10}>10 sec</option>
              <option value={30}>30 sec</option>
              <option value={60}>1 min</option>
              <option value={300}>5 min</option>
              <option value={1200}>20 min</option>
              <option value={3600}>1 hr</option>
              <option value={7200}>2 hr</option>
              <option value={86400}>24 hr</option>
              <option value={0}>permanent</option>
            </select>
          </label>
          <button
            onClick={setOverride}
            style={{
              padding: "3px 6px",
              background: "hsl(var(--cool))",
              color: "#fff",
              border: "1px solid hsl(var(--cool))",
              borderRadius: 2,
              cursor: "pointer",
              fontSize: 11,
              fontFamily: "inherit",
            }}
          >
            Set override
          </button>
        </div>
      ) : (
        <>
          {overridable && (
            <MenuItem
              onClick={() => setPromptOpen(true)}
              label={overridden ? "Change override…" : "Set override…"}
            />
          )}
          {overridable && overridden && (
            <MenuItem onClick={clearOverride} label="Clear override" danger />
          )}
          {canConnect && (
            <MenuItem onClick={() => setPickerOpen(true)} label="Connect to…" />
          )}
          {canConnect && ctx?.exposeProp && ctx.parentName && (
            <MenuItem
              onClick={() => {
                ctx.exposeProp?.(propUid);
                onClose();
              }}
              label={`Expose on ${ctx.parentName}`}
            />
          )}
          {exposed && ctx?.openDetails && (
            <MenuItem
              onClick={() => {
                ctx.openDetails?.(componentUid);
                onClose();
              }}
              label="Configure…"
            />
          )}
          {exposed && ctx?.unexposeProp && portOwner != null && (
            <MenuItem
              onClick={() => {
                ctx.unexposeProp?.(portOwner, propUid);
                onClose();
              }}
              label="Un-expose"
              danger
            />
          )}
        </>
      )}
      {pickerOpen && (
        <ConnectPicker
          x={x}
          y={y}
          sourceComponentUid={componentUid}
          sourcePropUid={propUid}
          sourceCategory={category === CATEGORY_OUTPUT ? "output" : "input"}
          onClose={() => {
            setPickerOpen(false);
            onClose();
          }}
        />
      )}
    </div>,
    wiresheetPortalRoot(),
  );
}

// Pops next to the property menu when the user clicks "Connect to…". Lists
// candidate target properties on every component in the current view (siblings
// of the source component), filtered by the source's category — outputs can
// only edge into inputs and vice versa. Self-component is skipped because an
// edge within the same component would be a no-op / cycle.
//
// On select: POST /edge with source on the OUTPUT side, target on the INPUT
// side (engine convention), then close. Topology event drives the reload so
// the new edge appears within a tick.
function ConnectPicker({
  x,
  y,
  sourceComponentUid,
  sourcePropUid,
  sourceCategory,
  onClose,
}: {
  x: number;
  y: number;
  sourceComponentUid: number;
  sourcePropUid: number;
  sourceCategory: "input" | "output";
  onClose: () => void;
}) {
  const [filter, setFilter] = useState("");
  // Which component is currently expanded. null = collapsed accordion (just
  // showing the component list). One-at-a-time so the picker stays compact.
  const [expanded, setExpanded] = useState<number | null>(null);
  // "New" mode: create a fresh component and connect to it, instead of picking
  // an existing one. Needs the editor's component types + create action.
  const ctx = useContext(CeWiresheetContext);
  const [creatingNew, setCreatingNew] = useState(false);
  // After picking a type in New mode, the freshly-created component is parked
  // here so the user can choose WHICH of its matching props to connect to,
  // instead of auto-wiring the first one.
  const [pendingNew, setPendingNew] = useState<Component | null>(null);
  // Keyboard navigation: index of the highlighted row (a property in Existing /
  // pick-input mode, a type in New mode). The filter is SHARED across Existing
  // and New, so switching (Tab / the +New button) keeps whatever you typed.
  const [highlight, setHighlight] = useState(0);
  const hlRef = useRef<HTMLButtonElement>(null);
  // Reset the highlight to the top whenever the candidate list changes (new
  // filter text, switching Existing↔New, or entering pick-input).
  useEffect(() => {
    setHighlight(0);
  }, [filter, creatingNew, pendingNew]);
  // Keep the highlighted row scrolled into view while arrowing through it.
  useEffect(() => {
    hlRef.current?.scrollIntoView({ block: "nearest" });
  }, [highlight, creatingNew]);

  // Dismiss on outside-click / Escape. Capture-phase pointerdown so React Flow's
  // pane (which stopImmediatePropagation's on press) can't swallow it. The
  // picker's root carries `data-ce-menu`, so clicks inside it don't dismiss.
  useEffect(() => {
    const dismiss = (e: MouseEvent) => {
      const el = e.target as Element | null;
      if (el && el.closest("[data-ce-menu]")) return;
      onClose();
    };
    const onEsc = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("pointerdown", dismiss, true);
    document.addEventListener("contextmenu", dismiss, true);
    document.addEventListener("keydown", onEsc);
    return () => {
      document.removeEventListener("pointerdown", dismiss, true);
      document.removeEventListener("contextmenu", dismiss, true);
      document.removeEventListener("keydown", onEsc);
    };
  }, [onClose]);
  // `sourceCategory` is the client-side direction ("input"/"output") of the
  // port the user is wiring FROM; the candidate category we want is the
  // opposite, as a numeric API category to compare against `p.category`.
  const wantCategory: PropertyCategory =
    sourceCategory === "output" ? CATEGORY_INPUT : CATEGORY_OUTPUT;

  // Edges can cross folders (per spec), so candidates include EVERY component
  // in the engine — not just siblings of the source. useStructural only holds
  // the current view's children, so we fetch the full tree on mount. Cached
  // inside the picker so reopening the same picker doesn't refetch.
  const [allComponents, setAllComponents] = useState<Component[] | null>(null);
  const [allEdges, setAllEdges] = useState<EdgeT[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const { getRootNodes } = await import("../lib/rest");
      try {
        const resp = await getRootNodes({ depth: -1, nested: true, withEdges: true });
        if (cancelled) return;
        const flat: Component[] = [];
        const walk = (c: Component) => {
          flat.push(c);
          c.children?.forEach(walk);
        };
        // resp.nodes[0] is the root; we want its descendants (root itself isn't
        // a target — its properties are engine-managed indicators).
        const root = resp.nodes[0];
        root?.children?.forEach(walk);
        setAllComponents(flat);
        setAllEdges(resp.edges ?? []);
      } catch {
        // Fall back to the current view if the global fetch fails.
        if (cancelled) return;
        setAllComponents([...useStructural.getState().components.values()]);
        setAllEdges([...useStructural.getState().edges.values()]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Inputs in dataflow take at most one incoming edge (the source of truth for
  // their value). When the user is wiring from an output, hide inputs that
  // already have something connected — they can't accept another. Outputs can
  // fan out to many targets, so the reverse direction needs no such filter.
  // Match by the target property's UID (the engine provides
  // `targetPropertyUid` on every edge) — an integer compare, no property-name
  // string matching. A set of all currently-targeted input prop uids.
  const taken =
    sourceCategory === "output" && allEdges ? takenInputUids(allEdges) : new Set<number>();

  // Look up the source component's parent so we can flag siblings. Use the
  // current view's structural cache — the source is always in scope there.
  const sourceComp = useStructural.getState().components.get(sourceComponentUid);
  const sourceParent = sourceComp?.parent;
  const sourceName = sourceComp?.name || "component";

  // Candidate grouping + tiering and the path-scope filter are pure (lib/connect,
  // tested). Parent (feed-through) → same level → children → elsewhere.
  const componentList = allComponents ?? [];
  const groups = buildConnectGroups(componentList, {
    sourceComponentUid,
    sourceParent,
    wantCategory,
    taken,
  });
  const filteredGroups = filterConnectGroups(groups, filter);
  const f = filter.trim(); // truthy = a filter is active (drives auto-expand)

  const create = async (target: { componentUid: number; propUid: number }) => {
    // Engine convention: source = output side, target = input side. Flip based
    // on which end the user is wiring from.
    const payload =
      sourceCategory === "output"
        ? {
            sourceUid: sourceComponentUid,
            sourcePropUid,
            targetUid: target.componentUid,
            targetPropUid: target.propUid,
          }
        : {
            sourceUid: target.componentUid,
            sourcePropUid: target.propUid,
            targetUid: sourceComponentUid,
            targetPropUid: sourcePropUid,
          };
    try {
      // Incremental edge add (append in-folder, reload only for cross-folder).
      // Falls back to a plain addEdge + WS reload if no context is present.
      if (ctx?.connectEdge) {
        await ctx.connectEdge(payload);
      } else {
        const { addEdge } = await import("../lib/rest");
        await addEdge(payload);
      }
    } catch (e) {
      console.error("add edge failed:", (e as Error).message);
    }
    onClose();
  };

  // If the filter narrows to a single property across all visible groups,
  // Enter creates that edge — fastest-path for keyboard users.
  const allFilteredProps = filteredGroups.flatMap((g) =>
    g.props.map((p) => ({ componentUid: g.componentUid, propUid: p.propUid })),
  );
  // Flat-index offset of each group's first prop, so one highlight index can
  // address the whole accordion. A group auto-opens when the highlight lands
  // inside it (see render), so arrowing down walks open groups for you.
  const groupPropOffsets: number[] = [];
  {
    let acc = 0;
    for (const g of filteredGroups) {
      groupPropOffsets.push(acc);
      acc += g.props.length;
    }
  }

  // "New" flow: create a component of `type` in the current folder, then connect
  // the source to its first matching-category property.
  const createNew = async (type: string) => {
    if (!ctx) return;
    // Connecting FROM an output → the new node is downstream (place it right);
    // FROM an input → the new node is upstream (place it left of the source).
    const side = sourceCategory === "output" ? "right" : "left";
    const c = await ctx.createComponent(type, { nearUid: sourceComponentUid, side });
    if (!c) {
      onClose();
      return;
    }
    const matching = Object.entries(c.properties ?? {})
      .filter(
        ([, p]) =>
          p.category === wantCategory && (p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL,
      )
      .map(([name, p]) => ({ uid: p.uid, name }));
    if (matching.length === 0) {
      onClose(); // nothing connectable — leave the new node placed
    } else if (matching.length === 1) {
      await create({ componentUid: c.uid, propUid: matching[0].uid }); // one option → wire it
    } else {
      // Multiple candidates → let the user pick which prop to connect to.
      setPendingNew(c);
      setFilter("");
    }
  };
  const nf = filter.trim().toLowerCase();
  const newTypes = (ctx?.componentTypes ?? []).filter(
    (t) => !nf || t.name.toLowerCase().includes(nf) || t.type.toLowerCase().includes(nf),
  );
  // Props of the just-created component the user can pick from (pick-input mode).
  const newProps = pendingNew
    ? Object.entries(pendingNew.properties ?? {})
        .filter(
          ([, p]) =>
            p.category === wantCategory && (p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL,
        )
        .map(([name, p]) => ({ uid: p.uid, name }))
    : [];
  const newPropsFiltered = nf
    ? newProps.filter((p) => p.name.toLowerCase().includes(nf))
    : newProps;

  // Position to the right of the parent menu where possible; clamp so it doesn't
  // run off-screen. The parent menu is at (x, y) and ~180px wide.
  const PICKER_W = 240;
  const left = Math.min(x + 184, window.innerWidth - PICKER_W - 8);
  const top = Math.min(y, window.innerHeight - 320);

  return createPortal(
    <div
      data-ce-menu
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        left,
        top,
        zIndex: 101,
        background: "hsl(var(--card))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 4,
        width: PICKER_W,
        maxHeight: 320,
        boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
        fontSize: 11,
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div style={{ padding: "6px 8px", borderBottom: "1px solid hsl(var(--border))" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 4 }}>
          {pendingNew ? (
            <>
              <button
                onClick={() => setPendingNew(null)}
                title="Back to component types"
                style={{
                  background: "transparent",
                  border: "none",
                  color: "hsl(var(--cool))",
                  cursor: "pointer",
                  fontSize: 13,
                  padding: 0,
                }}
              >
                ‹
              </button>
              <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, flex: 1 }}>
                {pendingNew.name} → pick {wantCategory === CATEGORY_INPUT ? "input" : "output"}
              </span>
            </>
          ) : creatingNew ? (
            <>
              <button
                onClick={() => setCreatingNew(false)}
                title="Back to existing components"
                style={{
                  background: "transparent",
                  border: "none",
                  color: "hsl(var(--cool))",
                  cursor: "pointer",
                  fontSize: 13,
                  padding: 0,
                }}
              >
                ‹
              </button>
              <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, flex: 1 }}>New component</span>
            </>
          ) : (
            <>
              <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, flex: 1 }}>
                Existing component → {wantCategory}…
              </span>
              {ctx && (
                <button
                  onClick={() => setCreatingNew(true)}
                  title="Create a new component and connect to it"
                  style={{
                    fontSize: 10,
                    padding: "1px 6px",
                    background: "hsl(var(--cool) / 0.18)",
                    color: "hsl(var(--cool))",
                    border: "1px solid hsl(var(--cool))",
                    borderRadius: 3,
                    cursor: "pointer",
                    fontFamily: "inherit",
                  }}
                >
                  + New
                </button>
              )}
            </>
          )}
        </div>
        <input
          autoFocus
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              onClose();
              return;
            }
            // Tab: from pick-input, go back to the type list; otherwise toggle
            // Existing ↔ New, keeping the typed filter.
            if (e.key === "Tab") {
              e.preventDefault();
              if (pendingNew) setPendingNew(null);
              else if (ctx) setCreatingNew((v) => !v);
              return;
            }
            const len = pendingNew
              ? newPropsFiltered.length
              : creatingNew
                ? newTypes.length
                : allFilteredProps.length;
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setHighlight((h) => Math.min(h + 1, Math.max(0, len - 1)));
              return;
            }
            if (e.key === "ArrowUp") {
              e.preventDefault();
              setHighlight((h) => Math.max(0, h - 1));
              return;
            }
            if (e.key === "Enter") {
              e.preventDefault();
              if (pendingNew) {
                const p = newPropsFiltered[highlight];
                if (p) void create({ componentUid: pendingNew.uid, propUid: p.uid });
              } else if (creatingNew) {
                const t = newTypes[highlight];
                if (t) void createNew(t.type);
              } else {
                const p = allFilteredProps[highlight];
                if (p) void create(p);
              }
              return;
            }
            e.stopPropagation();
          }}
          placeholder={
            pendingNew ? "filter inputs…" : creatingNew ? "filter types…   ⇥ existing" : "filter…   ⇥ new"
          }
          style={{
            width: "100%",
            background: "hsl(var(--background))",
            color: "hsl(var(--foreground))",
            border: "1px solid hsl(var(--border))",
            borderRadius: 2,
            padding: "3px 6px",
            fontSize: 11,
            fontFamily: "var(--font-mono)",
            boxSizing: "border-box",
            outline: "none",
          }}
        />
      </div>
      <div style={{ flex: 1, overflowY: "auto" }}>
        {pendingNew ? (
          newPropsFiltered.length === 0 ? (
            <div style={{ padding: "10px 8px", color: "hsl(var(--muted-foreground))", fontSize: 11 }}>
              no matching {wantCategory === CATEGORY_INPUT ? "inputs" : "outputs"}
            </div>
          ) : (
            newPropsFiltered.map((p, i) => (
              <button
                key={p.uid}
                ref={i === highlight ? hlRef : undefined}
                onClick={() => create({ componentUid: pendingNew.uid, propUid: p.uid })}
                style={{
                  display: "block",
                  width: "100%",
                  textAlign: "left",
                  padding: "5px 8px",
                  background: i === highlight ? "hsl(var(--cool) / 0.18)" : "transparent",
                  color: "hsl(var(--foreground))",
                  border: "none",
                  cursor: "pointer",
                  fontSize: 11,
                  fontFamily: "var(--font-mono)",
                }}
                onMouseEnter={(e) => (e.currentTarget.style.background = "hsl(var(--secondary))")}
                onMouseLeave={(e) =>
                  (e.currentTarget.style.background = i === highlight ? "hsl(var(--cool) / 0.18)" : "transparent")
                }
              >
                {p.name}
              </button>
            ))
          )
        ) : creatingNew ? (
          newTypes.length === 0 ? (
            <div style={{ padding: "10px 8px", color: "hsl(var(--muted-foreground))", fontSize: 11 }}>
              {ctx ? "no matching types" : "unavailable"}
            </div>
          ) : (
            newTypes.map((t, i) => (
              <button
                key={t.type}
                ref={i === highlight ? hlRef : undefined}
                onClick={() => createNew(t.type)}
                style={{
                  display: "flex",
                  width: "100%",
                  textAlign: "left",
                  padding: "5px 8px",
                  background: i === highlight ? "hsl(var(--cool) / 0.18)" : "transparent",
                  color: "hsl(var(--foreground))",
                  border: "none",
                  cursor: "pointer",
                  fontSize: 11,
                  fontFamily: "var(--font-mono)",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: 6,
                }}
                onMouseEnter={(e) => (e.currentTarget.style.background = "hsl(var(--secondary))")}
                onMouseLeave={(e) =>
                  (e.currentTarget.style.background = i === highlight ? "hsl(var(--cool) / 0.18)" : "transparent")
                }
              >
                <span>{t.name}</span>
                <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 9 }}>{t.group}</span>
              </button>
            ))
          )
        ) : filteredGroups.length === 0 ? (
          <div style={{ padding: "10px 8px", color: "hsl(var(--muted-foreground))", fontSize: 11 }}>
            {allComponents == null ? "loading…" : "no candidates"}
          </div>
        ) : (
          filteredGroups.map((g, idx) => {
            // Under an active filter every visible group is auto-expanded —
            // user already pre-narrowed, no need to make them click again. Also
            // auto-open the group the keyboard highlight currently sits in, so
            // arrowing down reveals props as you reach them.
            const base = groupPropOffsets[idx];
            const containsHl = highlight >= base && highlight < base + g.props.length;
            const isOpen = f ? true : expanded === g.componentUid || containsHl;
            // Section header whenever the tier changes (parent / same level /
            // inside <source> / other folders).
            const prev = idx > 0 ? filteredGroups[idx - 1] : null;
            const tier = connectTier(g);
            const showSection = tier !== (prev ? connectTier(prev) : -1);
            const sectionLabel =
              tier === 0
                ? "parent"
                : tier === 1
                  ? "same level"
                  : tier === 2
                    ? `inside ${sourceName}`
                    : "other folders";
            // Folder-chain subtitle for "other folders" rows. Drop the
            // component's own name segment, then the leading "root" so
            // root/add1 reads as /add1.
            const folderPath = g.path.replace(/\/[^/]*$/, "").replace(/^root/, "");
            const showPath = tier === 3 && folderPath !== "";
            return (
              <div key={g.componentUid}>
                {showSection && (
                  <div
                    style={{
                      padding: "6px 8px 2px 8px",
                      color: "hsl(var(--muted-foreground))",
                      fontSize: 9,
                      textTransform: "uppercase",
                      letterSpacing: 0.4,
                      borderTop: idx > 0 ? "1px solid hsl(var(--border))" : "none",
                      marginTop: idx > 0 ? 2 : 0,
                    }}
                  >
                    {sectionLabel}
                  </div>
                )}
                <button
                  onClick={() =>
                    setExpanded((cur) => (cur === g.componentUid ? null : g.componentUid))
                  }
                  style={{
                    display: "flex",
                    width: "100%",
                    textAlign: "left",
                    padding: "5px 8px",
                    background: "transparent",
                    color: "hsl(var(--foreground))",
                    border: "none",
                    cursor: "pointer",
                    fontSize: 11,
                    fontFamily: "var(--font-mono)",
                    alignItems: "center",
                    gap: 6,
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = "hsl(var(--secondary))")}
                  onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
                >
                  <span
                    style={{
                      display: "flex",
                      alignItems: "center",
                      color: "hsl(var(--muted-foreground))",
                      flexShrink: 0,
                    }}
                  >
                    {isOpen ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
                  </span>
                  <span
                    style={{
                      flex: 1,
                      minWidth: 0,
                      display: "flex",
                      flexDirection: "column",
                      overflow: "hidden",
                    }}
                  >
                    <span
                      style={{
                        color: "hsl(var(--cool))",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {g.componentName}
                      {g.isParent && (
                        <span
                          style={{
                            marginLeft: 6,
                            fontSize: 8,
                            textTransform: "uppercase",
                            letterSpacing: 0.4,
                            color: "hsl(var(--amber))",
                            border: "1px solid hsl(var(--amber) / 0.4)",
                            background: "hsl(var(--amber) / 0.2)",
                            borderRadius: 3,
                            padding: "0 4px",
                          }}
                        >
                          parent
                        </span>
                      )}
                    </span>
                    {showPath && (
                      <span
                        style={{
                          color: "hsl(var(--muted-foreground))",
                          fontSize: 9,
                          overflow: "hidden",
                          textOverflow: "ellipsis",
                          whiteSpace: "nowrap",
                        }}
                        title={g.path}
                      >
                        {folderPath}
                      </span>
                    )}
                  </span>
                  <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>{g.props.length}</span>
                </button>
                {isOpen && (
                  <div style={{ paddingBottom: 2 }}>
                    {g.props.map((p, pi) => {
                      const isHl = base + pi === highlight;
                      return (
                        <button
                          key={p.propUid}
                          ref={isHl ? hlRef : undefined}
                          onClick={() =>
                            create({ componentUid: g.componentUid, propUid: p.propUid })
                          }
                          style={{
                            display: "block",
                            width: "100%",
                            textAlign: "left",
                            padding: "3px 8px 3px 28px",
                            background: isHl ? "hsl(var(--cool) / 0.18)" : "transparent",
                            color: "hsl(var(--foreground))",
                            border: "none",
                            cursor: "pointer",
                            fontSize: 11,
                            fontFamily: "var(--font-mono)",
                          }}
                          onMouseEnter={(e) => (e.currentTarget.style.background = "hsl(var(--border))")}
                          onMouseLeave={(e) =>
                            (e.currentTarget.style.background = isHl ? "hsl(var(--cool) / 0.18)" : "transparent")
                          }
                        >
                          {p.propName}
                        </button>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>
    </div>,
    wiresheetPortalRoot(),
  );
}

function MenuItem({ onClick, label, danger }: { onClick: () => void; label: string; danger?: boolean }) {
  return (
    <button
      onClick={onClick}
      style={{
        display: "block",
        width: "100%",
        textAlign: "left",
        padding: "5px 8px",
        background: "transparent",
        color: danger ? "hsl(var(--crit))" : "hsl(var(--foreground))",
        border: "none",
        borderRadius: 2,
        cursor: "pointer",
        fontSize: 11,
        fontFamily: "inherit",
      }}
      onMouseEnter={(e) => (e.currentTarget.style.background = "hsl(var(--border))")}
      onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
    >
      {label}
    </button>
  );
}

// Inline value editor for inputs / config properties. Click → text input opens with
// the current value selected. Enter or blur → PATCH /nodes/uid/{uid}. Escape → cancel.
//
// Why direct PATCH rather than going through a parent callback: keeps the editor
// self-contained, no prop drilling, and the topology event the engine fires in
// response refreshes the live value via the normal value-plane path.
function PropertyValueEditor({
  componentUid,
  propName,
  value,
  dataType,
  facet,
}: {
  componentUid: number;
  propName: string;
  value: DecodedValue | undefined;
  dataType: PropertyDataType;
  facet?: PropFacet;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<string>("");
  const [rect, setRect] = useState<DOMRect | null>(null);

  const display = fmtValueFacet(value, dataType, facet);
  const fullTitle = value == null ? "" : typeof value === "string" ? value : display;
  const start = (e?: React.MouseEvent) => {
    if (e) setRect(e.currentTarget.getBoundingClientRect());
    setDraft(value == null ? "" : typeof value === "string" ? value : String(value));
    setEditing(true);
  };
  const cancel = () => {
    setEditing(false);
    setRect(null);
  };

  // Dismiss the string popup on an outside click.
  useEffect(() => {
    if (!editing || dataType !== DATATYPE_STRING) return;
    const onDown = (ev: PointerEvent) => {
      if (!document.getElementById("fb-str-popup")?.contains(ev.target as Node)) cancel();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
  }, [editing, dataType]);

  const commit = async () => {
    setEditing(false);
    setRect(null);
    const raw = draft.trim();
    if (raw === "") return;
    // Parse the draft according to the property's dataType. Strings go through as-is.
    let parsed: string | number | boolean;
    if (dataType === DATATYPE_BOOL) {
      const lower = raw.toLowerCase();
      parsed = lower === "true" || lower === "1" || lower === "yes";
    } else if (dataType === DATATYPE_STRING) {
      parsed = raw;
    } else {
      const n = Number(raw);
      if (!Number.isFinite(n)) return;
      parsed = n;
    }
    try {
      const { updateNode } = await import("../lib/rest");
      await updateNode(componentUid, { properties: { [propName]: { value: parsed } } });
    } catch (e) {
      console.error("update value failed:", (e as Error).message);
    }
  };

  if (editing) {
    const stop = (e: React.SyntheticEvent) => e.stopPropagation();
    // Aliased value (bool or int enum) → a dropdown of the alias labels, writing
    // back the native value (the code; bool → code 1/0).
    if (facet?.aliases && facet.aliases.length) {
      const cur =
        value === true ? 1 : value === false ? 0 : typeof value === "number" ? value : Number(value);
      return (
        <select
          autoFocus
          className="nodrag"
          value={String(cur)}
          onChange={(e) => commitAlias(Number(e.target.value))}
          onKeyDown={(e) => {
            if (e.key === "Escape") setEditing(false);
            e.stopPropagation();
          }}
          onBlur={() => setEditing(false)}
          onClick={stop}
          onPointerDown={stop}
          style={editorInputStyle}
        >
          {facet.aliases.map((a) => (
            <option key={a.code} value={String(a.code)}>
              {a.label}
            </option>
          ))}
        </select>
      );
    }
    // `nodrag` is React Flow's opt-out class: nodes won't start a drag from
    // pointer events on elements carrying it. Critical for native form
    // controls (especially <select>) because the OS dropdown captures the
    // pointer events — RF sees pointerdown but never the pointerup, leaving
    // its drag state stuck and the node ends up following the cursor after
    // the user picks an option.
    if (dataType === DATATYPE_BOOL) {
      return (
        <select
          autoFocus
          className="nodrag"
          value={draft}
          onChange={(e) => commitWith(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") setEditing(false);
            e.stopPropagation();
          }}
          onBlur={() => setEditing(false)}
          onClick={stop}
          onPointerDown={stop}
          style={editorInputStyle}
        >
          <option value="true">true</option>
          <option value="false">false</option>
        </select>
      );
    }
    // Strings edit in a popup so long text isn't constrained to the row width.
    if (dataType === DATATYPE_STRING) {
      return (
        <>
          <span style={valueDisplayStyle} title={fullTitle}>{display || "—"}</span>
          {rect &&
            createPortal(
              <div
                id="fb-str-popup"
                className="nodrag"
                onClick={(e) => e.stopPropagation()}
                onPointerDown={(e) => e.stopPropagation()}
                style={{
                  position: "fixed",
                  left: Math.min(rect.left, window.innerWidth - 280),
                  top: Math.min(rect.bottom + 4, window.innerHeight - 180),
                  zIndex: 200,
                  width: 260,
                  background: "hsl(var(--card))",
                  border: "1px solid hsl(var(--border))",
                  borderRadius: 6,
                  boxShadow: "0 8px 24px rgba(0,0,0,0.6)",
                  padding: 8,
                  display: "flex",
                  flexDirection: "column",
                  gap: 6,
                }}
              >
                <textarea
                  autoFocus
                  value={draft}
                  rows={4}
                  onChange={(e) => setDraft(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Escape") cancel();
                    else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) commit();
                    e.stopPropagation();
                  }}
                  style={{ ...overrideInputStyle, width: "100%", resize: "vertical" }}
                />
                <div style={{ display: "flex", gap: 6, justifyContent: "flex-end", alignItems: "center" }}>
                  <CopyButton text={draft} />
                  <span style={{ flex: 1 }} />
                  <button onClick={cancel} style={popupBtn(false)}>cancel</button>
                  <button onClick={commit} style={popupBtn(true)}>save</button>
                </div>
              </div>,
              wiresheetPortalRoot(),
            )}
        </>
      );
    }
    return (
      <input
        autoFocus
        // Highlight the existing value on open so typing replaces it immediately.
        onFocus={(e) => e.currentTarget.select()}
        className="nodrag"
        type={dataType === DATATYPE_NUMBER ? "number" : "text"}
        // Sensible step for the number spinner — full integer step by default,
        // user can still type any decimal. inputMode keeps mobile keyboards
        // sane.
        inputMode={dataType === DATATYPE_NUMBER ? "decimal" : undefined}
        step={dataType === DATATYPE_NUMBER ? "any" : undefined}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          else if (e.key === "Escape") setEditing(false);
          e.stopPropagation();
        }}
        onBlur={commit}
        onClick={stop}
        onPointerDown={stop}
        style={editorInputStyle}
      />
    );
  }
  return (
    <span
      // No `nodrag` / pointerdown-stop here: a press-and-drag starting on a
      // value should move the node like any other body grab. A plain click
      // (movement < nodeDragThreshold) still falls through to onClick → edit.
      onClick={(e) => {
        e.stopPropagation();
        start(e);
      }}
      style={{
        ...valueDisplayStyle,
        color: dataType === DATATYPE_BOOL ? COLOR_BOOL : "hsl(var(--foreground))",
        // Bigger click target for editable values: reserve a minimum width so a
        // short value (e.g. "0") is still easy to hit, fill the row height, and
        // right-align so it still reads flush to the edge. Long values still
        // ellipsize (maxWidth from valueDisplayStyle, inline-block preserved).
        minWidth: 48,
        textAlign: "right",
        height: 18,
        lineHeight: "18px",
        boxSizing: "border-box",
        padding: "0 6px",
      }}
      title={fullTitle || "click to edit"}
    >
      {display || "—"}
    </span>
  );

  // Inline helper that commits a specific raw value (used by the bool select
  // since onChange fires with the new value before setDraft would land).
  async function commitWith(raw: string) {
    setDraft(raw);
    setEditing(false);
    let parsed: string | number | boolean;
    if (dataType === DATATYPE_BOOL) {
      parsed = raw === "true";
    } else if (dataType === DATATYPE_STRING) {
      parsed = raw;
    } else {
      const n = Number(raw);
      if (!Number.isFinite(n)) return;
      parsed = n;
    }
    try {
      const { updateNode } = await import("../lib/rest");
      await updateNode(componentUid, { properties: { [propName]: { value: parsed } } });
    } catch (e) {
      console.error("update value failed:", (e as Error).message);
    }
  }

  // Commit an aliased selection: write the native value (bool → code 1/0,
  // otherwise the int code itself).
  async function commitAlias(code: number) {
    setEditing(false);
    const parsed: number | boolean = dataType === DATATYPE_BOOL ? code === 1 : code;
    try {
      const { updateNode } = await import("../lib/rest");
      await updateNode(componentUid, { properties: { [propName]: { value: parsed } } });
    } catch (e) {
      console.error("update value failed:", (e as Error).message);
    }
  }
}

// Value cells truncate with an ellipsis so long strings never overflow the node;
// the full value is on the `title` tooltip (and editable in the popup).
const valueDisplayStyle: React.CSSProperties = {
  fontVariantNumeric: "tabular-nums",
  cursor: "text",
  padding: "0 2px",
  borderRadius: 2,
  maxWidth: 130,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  display: "inline-block",
  verticalAlign: "bottom",
};

const popupBtn = (primary: boolean): React.CSSProperties => ({
  padding: "3px 10px",
  fontSize: 11,
  borderRadius: 3,
  cursor: "pointer",
  color: primary ? "hsl(var(--cool))" : "hsl(var(--muted-foreground))",
  background: primary ? "hsl(var(--cool) / 0.18)" : "transparent",
  border: `1px solid ${primary ? "hsl(var(--cool))" : "hsl(var(--border))"}`,
});

const editorInputStyle: React.CSSProperties = {
  width: 90,
  background: "hsl(var(--background))",
  color: "hsl(var(--foreground))",
  border: "1px solid hsl(var(--cool))",
  borderRadius: 2,
  padding: "0 4px",
  fontFamily: "inherit",
  fontSize: 11,
  textAlign: "right",
  outline: "none",
};

const overrideInputStyle: React.CSSProperties = {
  background: "hsl(var(--background))",
  color: "hsl(var(--foreground))",
  border: "1px solid hsl(var(--cool))",
  borderRadius: 2,
  padding: "3px 6px",
  fontFamily: "var(--font-mono)",
  fontSize: 11,
  outline: "none",
};

function StatusDot({ color, text }: { color: string; text: string }) {
  const [hover, setHover] = useState(false);
  return (
    <span
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        position: "relative",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        flexShrink: 0,
      }}
    >
      <span
        style={{
          width: 8,
          height: 8,
          borderRadius: 4,
          background: color,
          boxShadow: "0 0 0 1px rgba(0,0,0,0.4)",
          display: "block",
        }}
      />
      {hover && (
        <span
          style={{
            position: "absolute",
            top: "100%",
            right: 0,
            marginTop: 6,
            padding: "3px 7px",
            background: "hsl(var(--background))",
            border: "1px solid " + color,
            borderRadius: 3,
            color: "hsl(var(--foreground))",
            fontSize: 10,
            fontFamily: "var(--font-mono)",
            whiteSpace: "nowrap",
            zIndex: 50,
            pointerEvents: "none",
            boxShadow: "0 2px 6px rgba(0,0,0,0.5)",
          }}
        >
          {text || "—"}
        </span>
      )}
    </span>
  );
}

// The engine emits the status property's VALUE as a JSON object serialized to
// a string. An empty object "{}" means the component is healthy. A populated
// object carries fields like { error: "...", warning: "..." } — for now we
// surface the first non-empty string field as a hint. Logged as API_GAPS #8
// because the encoding is ambiguous: it's a string-shaped object, the client
// has to JSON.parse to know whether to render "ok" or an error label.
function parseStatus(raw: unknown): string {
  if (raw == null) return "";
  if (typeof raw !== "string") {
    // Engine could conceivably stop double-encoding one day — accept a real
    // object too without bothering the parser.
    if (typeof raw === "object") return summarizeStatusObject(raw);
    return String(raw);
  }
  // Strings that match a known label (engine might switch to plain labels
  // later) are returned as-is.
  const t = raw.trim();
  if (t === "" || t === "{}") return "";
  try {
    const obj = JSON.parse(t);
    if (obj == null) return "";
    if (typeof obj === "string") return obj;
    if (typeof obj === "object") return summarizeStatusObject(obj);
  } catch {
    // Not JSON — treat as plain label.
  }
  return t;
}

function summarizeStatusObject(obj: object): string {
  for (const [k, v] of Object.entries(obj)) {
    if (typeof v === "string" && v.trim() !== "") return `${k}: ${v}`;
    if (typeof v === "boolean" && v) return k;
    if (typeof v === "number" && v !== 0) return `${k}=${v}`;
  }
  return "";
}

// Map a status value to an indicator color. Unknown / empty / "NONE" reads as healthy.
// Any non-empty value the engine emits that isn't recognised falls into the "other"
// bucket and shows a neutral grey — better than nothing while we learn the vocabulary.
function statusColorFor(s: string): { bg: string; label: string } {
  const v = s.toUpperCase();
  if (!v || v === "NONE" || v === "OK") return { bg: "hsl(var(--green))", label: v || "ok" };
  if (v === "STALE") return { bg: "hsl(var(--amber))", label: "stale" };
  if (v === "OVERRIDDEN") return { bg: "hsl(var(--cool))", label: "overridden" };
  if (v === "ERROR" || v === "FAULT" || v === "DOWN") return { bg: "hsl(var(--crit))", label: v.toLowerCase() };
  return { bg: "hsl(var(--muted-foreground))", label: s };
}

interface InnerProps {
  data: FunctionBlockData;
  // React Flow passes `selected` to every node component. We use it to paint the
  // selection highlight ourselves, since custom node types don't inherit the default
  // RF .selected outline.
  selected?: boolean;
}

// A single property row. PERF: subscribes to its OWN value + status uid, so a
// value frame re-renders only the rows whose values changed — not the whole node.
// memo'd so a parent re-render (structural change) with the same row props is a
// no-op for unchanged rows.
// Read-only value display. For string values (e.g. outputs) a click opens a
// popup showing the full text — outputs can be long and aren't editable, so the
// truncated cell + tooltip isn't enough to actually read them.
function ReadonlyValue({
  value,
  dataType,
  facet,
}: {
  value: DecodedValue | undefined;
  dataType: PropertyDataType;
  facet?: PropFacet;
}) {
  const [rect, setRect] = useState<DOMRect | null>(null);
  const display = fmtValueFacet(value, dataType, facet);
  const isString = dataType === DATATYPE_STRING || typeof value === "string";
  const full = typeof value === "string" ? value : display;

  useEffect(() => {
    if (!rect) return;
    const onDown = (ev: PointerEvent) => {
      if (!document.getElementById("fb-read-popup")?.contains(ev.target as Node)) setRect(null);
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
  }, [rect]);

  if (!isString) {
    return (
      <span style={{ ...valueDisplayStyle, cursor: "default", color: dataType === DATATYPE_BOOL ? COLOR_BOOL : "hsl(var(--foreground))" }} title={DATATYPE_LABEL[dataType]}>
        {display || "—"}
      </span>
    );
  }
  return (
    <>
      <span
        onClick={(e) => { e.stopPropagation(); setRect(e.currentTarget.getBoundingClientRect()); }}
        title="click to read"
        style={{ ...valueDisplayStyle, cursor: "zoom-in", color: "hsl(var(--foreground))" }}
      >
        {display || "—"}
      </span>
      {rect &&
        createPortal(
          <div
            id="fb-read-popup"
            className="nodrag"
            onClick={(e) => e.stopPropagation()}
            onPointerDown={(e) => e.stopPropagation()}
            style={{
              position: "fixed",
              left: Math.min(rect.left, window.innerWidth - 300),
              top: Math.min(rect.bottom + 4, window.innerHeight - 240),
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
    </>
  );
}

const ValueRow = memo(function ValueRow({
  row: p,
  i,
  componentUid,
  initialFlags,
}: {
  row: PropRow;
  i: number;
  componentUid: number;
  initialFlags: number;
}) {
  const v = useValues((s) => s.values.get(p.uid));
  const liveFlags = useStatusFlags((s) => s.flags.get(p.uid));
  // Exposed ports read presentation LIVE from the child's streamed __facets.
  const facetV = useValues((s) =>
    p.facetPropUid != null ? s.values.get(p.facetPropUid) : undefined,
  );

  const isInput = p.category === CATEGORY_INPUT;
  const isOutput = p.category === CATEGORY_OUTPUT;
  let rowFacet = p.facet;
  if (p.exposed && p.facetPropUid != null && p.exposedComponent != null && typeof facetV === "string") {
    const live = facetFor(p.exposedComponent, facetV).get(p.uid);
    if (live) rowFacet = { ...p.facet, ...live, label: live.label ?? p.facet?.label };
  }
  const flags = liveFlags ?? initialFlags;
  const overridden = (flags & STATUS_OVERRIDDEN) !== 0;
  const editable = !p.exposed && (isInput || p.category === CATEGORY_CONFIG);
  const rowTitle = `${p.name} — prop uid ${p.uid} · component uid ${
    p.exposed ? (p.exposedComponent ?? "?") : componentUid
  }`;

  return (
    <div
      data-row-uid={p.uid}
      title={rowTitle}
      style={{
        position: "absolute",
        left: 0,
        right: 0,
        top: TITLE_H + i * ROW_H,
        height: ROW_H,
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        padding: "0 12px",
        fontSize: 11,
        fontFamily: "var(--font-mono)",
        background: overridden ? "rgba(245,158,11,0.08)" : "transparent",
      }}
    >
      <span
        style={{
          color: isInput ? "hsl(var(--muted-foreground))" : isOutput ? "hsl(var(--foreground))" : "hsl(var(--muted-foreground))",
          display: "flex",
          alignItems: "center",
          gap: 4,
        }}
      >
        {p.exposed && (
          <span
            style={{ display: "flex", alignItems: "center", color: "hsl(var(--muted-foreground))" }}
            title="exposed from a child"
          >
            <CornerDownRight size={11} strokeWidth={2} />
          </span>
        )}
        <span title={rowFacet?.label ? p.name : undefined}>{rowFacet?.label ?? p.name}</span>
        {p.category === CATEGORY_CONFIG ? " (cfg)" : ""}
        {overridden && (
          <span
            title="overridden"
            style={{
              fontSize: 9,
              padding: "0 4px",
              background: "hsl(var(--amber))",
              color: "hsl(var(--background))",
              borderRadius: 2,
              fontWeight: 600,
            }}
          >
            OVR
          </span>
        )}
      </span>
      {editable ? (
        <PropertyValueEditor
          componentUid={componentUid}
          propName={p.name}
          value={v}
          dataType={p.dataType}
          facet={rowFacet}
        />
      ) : p.dataType === DATATYPE_STRING || typeof v === "string" ? (
        // Strings get the stateful read-popup; everything else is a plain span
        // (no per-cell hooks — keeps the all-node re-render cheap).
        <ReadonlyValue value={v} dataType={p.dataType} facet={rowFacet} />
      ) : (
        <span
          style={{ ...valueDisplayStyle, cursor: "default", color: p.dataType === DATATYPE_BOOL ? COLOR_BOOL : "hsl(var(--foreground))" }}
          title={DATATYPE_LABEL[p.dataType]}
        >
          {fmtValueFacet(v, p.dataType, rowFacet) || "—"}
        </span>
      )}
    </div>
  );
});

function FunctionBlockInner({ data, selected }: InnerProps) {
  // ALL hooks run unconditionally before any early-return branch — otherwise React
  // loses its hook-order invariant and throws "Rendered more hooks than during the
  // previous render."
  const schemaV = useSchemaVersion((s) => s.version);
  const ctx = useContext(CeWiresheetContext);
  // Subscribe to the live REST component so structural changes re-render the
  // block without a manual reload. Default Object.is equality only re-renders when
  // THIS uid's entry changes (upsertComponent swaps just that one entry).
  const restComp = useStructural((s) => s.components.get(data.componentUid));
  // Prop uids that are an edge endpoint — used to keep wired props from hiding.
  const linkedProps = useStructural((s) => s.linkedProps);
  // Level-of-detail: true when zoomed out far enough that values aren't legible.
  // Boolean selector → this node only re-renders when CROSSING the threshold,
  // not on every zoom delta.
  const lod = useRfStore((s) => s.transform[2] < LOD_ZOOM);
  // PERF: the node body does NOT subscribe to the value/status stream — each prop
  // ROW (ValueRow) subscribes to its OWN uid, so a value change re-renders just
  // that row, not the whole node (which would re-create every row element every
  // frame). The parent only watches its own __facets value, to detect a
  // cross-session facet edit and trigger a reload (below).
  const ownFacetUid = restComp?.properties[FACET_PROP]?.uid;
  const liveFacetRaw = useValues((s) => {
    if (ownFacetUid == null) return undefined;
    const v = s.values.get(ownFacetUid);
    return typeof v === "string" ? v : undefined;
  });
  const [menu, setMenu] = useState<{
    x: number;
    y: number;
    propName: string;
    propUid: number;
    category: PropertyCategory;
    dataType: PropertyDataType;
    currentValue: DecodedValue | undefined;
    overridden: boolean;
    exposed?: boolean;
    exposedComponent?: number;
    portOwner?: number;
  } | null>(null);
  void schemaV;

  // Collaborators (other sessions) who currently have THIS component selected.
  // Per-component selector with shallow equality. CRITICAL: the selector must
  // return PRIMITIVES, not fresh objects. useShallow compares array elements
  // with Object.is — new {name,color} objects every call never match, so it
  // returns a new array every render → "getSnapshot should be cached" infinite
  // loop (blanks the tree). Encoding each as a "color\tname" string makes the
  // shallow compare value-based and stable; we split in render.
  const otherSelectorKeys = usePresence(
    useShallow((s) => {
      const out: string[] = [];
      for (const c of s.collaborators.values()) {
        if (c.state.selectedComponents?.includes(data.componentUid)) {
          const name = c.state.userName ?? c.sessionId.slice(0, 6);
          out.push(`${PRESENCE_PALETTE[c.colorIdx]}\t${name}`);
        }
      }
      return out;
    }),
  );
  const otherSelectors = otherSelectorKeys.map((k) => {
    const [color, name] = k.split("\t");
    return { color, name };
  });

  // Structural derivation (rows, node height, status indicator) — pure function
  // of the REST component + the WS schema. Memoized so the per-FRAME value/
  // status re-renders (chatty math nodes re-render ~10×/s) DON'T rebuild the
  // row list, re-filter by category, or re-scan for the status prop every time.
  // Only a real structural change (props added/removed → restComp identity
  // swaps) or a schema arrival (dataType table fills → schemaV bumps) recomputes
  // this. The live value/flag reads stay in the row JSX below.
  // Cross-session facet sync, done SAFELY: the structural memo below renders from
  // the REST copy of __facets (authoritative — never clobbered by a stale/empty
  // stream value, which was breaking expose). The live stream is used ONLY as a
  // "something changed" trigger: __facets is an input string, so its value streams
  // into our value map keyed by its own uid; when that streamed value TRANSITIONS
  // (another session edited the facet), request a debounced scope reload so REST
  // refreshes structural and the rows/ports/edges rebuild consistently. Compared
  // against the previous STREAMED value (not structural) so it can't loop.
  const prevFacetRaw = useRef<string | null>(null);
  useEffect(() => {
    if (liveFacetRaw == null) return;
    if (prevFacetRaw.current === null) {
      prevFacetRaw.current = liveFacetRaw; // seed; don't fire on first sight
      return;
    }
    if (liveFacetRaw !== prevFacetRaw.current) {
      prevFacetRaw.current = liveFacetRaw;
      ctx?.requestReload?.();
    }
  }, [liveFacetRaw, ctx]);

  const structural = useMemo(() => {
    if (!restComp) return null;
    // User-facing = normal role (the `system` bool is gone; systemRole != 0
    // means an engine-managed slot).
    const isUserFacing = (p: Property) => (p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL;
    const entries = Object.entries(restComp.properties);
    // Parse this component's __facet (cached by raw string) and attach each
    // prop's metadata to its row. Hidden rows are dropped; `order` sorts within
    // each category group (stable for rows without it).
    // Authoritative: the REST copy of __facets (cached by raw string). The live
    // stream only triggers a reload (above) — it never sources the render, so a
    // stale/empty streamed value can't blank out exposed ports.
    const facet = facetFor(restComp.uid, rawFacet(restComp.properties));
    const mappedRows: PropRow[] = entries
      .filter(([, p]) => isUserFacing(p))
      .map(([name, p]) => ({
        uid: p.uid,
        name,
        category: p.category,
        dataType: propertyDataType.get(p.uid) ?? inferDataType(p.value),
        systemRole: p.systemRole,
        facet: withChoices(facet.get(p.uid), restComp.type, name),
      }));
    // A wired (linked) prop is never hidden — keep it visible even if its facet
    // says hidden (you can't hide an active connection).
    const hiddenCount = mappedRows.filter((r) => r.facet?.hidden && !linkedProps.has(r.uid)).length;
    const userRows = mappedRows.filter((r) => !r.facet?.hidden || linkedProps.has(r.uid));
    // Exposed ports: child props this component projects as its own input/output
    // ports (see FACET_DESIGN.md §9). uid = the child prop uid (its handle id),
    // dataType from the global schema index, value via the subscription above.
    // Read-only here — you edit the real value inside the child.
    const portRows: PropRow[] = exposedPorts(facet).map((ep) => ({
      uid: ep.childUid,
      // user label → maintainer-resolved owner-prop name (`m`) → uid fallback.
      name: ep.facet.label ?? ep.facet.name ?? `#${ep.childUid}`,
      category: ep.side === "input" ? CATEGORY_INPUT : CATEGORY_OUTPUT,
      dataType: propertyDataType.get(ep.childUid) ?? inferDataType(undefined),
      facet: ep.facet,
      exposed: true,
      exposedComponent: ep.facet.childComponent,
      facetPropUid: ep.facet.facetProp,
    }));
    const allRows = [...userRows, ...portRows];
    const byOrder = (a: PropRow, b: PropRow) =>
      (a.facet?.order ?? Number.MAX_SAFE_INTEGER) - (b.facet?.order ?? Number.MAX_SAFE_INTEGER);
    const rows: PropRow[] = [
      ...allRows.filter((r) => r.category === CATEGORY_OUTPUT).sort(byOrder),
      ...allRows.filter((r) => r.category === CATEGORY_INPUT).sort(byOrder),
      ...allRows.filter((r) => r.category === CATEGORY_CONFIG).sort(byOrder),
    ];
    const statusEntry = entries.find(([, p]) => p.systemRole === ROLE_STATUS);
    const statusText = parseStatus(statusEntry?.[1].value);
    return {
      rows,
      // + ROW_H for the bottom lip (drill-in button + action marker).
      nodeH: TITLE_H + rows.length * ROW_H + ROW_H,
      kind: restComp.type,
      statusText,
      statusColor: statusColorFor(statusText),
      statusPropExists: statusEntry != null,
      hiddenCount,
    };
    // schemaV in deps: when the WS schema fills propertyDataType, recompute
    // dataTypes. restComp identity swaps on any structural change.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [restComp, schemaV, linkedProps]);

  // Count this render for diagnostics. A re-render storm (every node
  // re-rendering on every frame) shows up as renders/sec ≈ frames/sec ×
  // node-count in the DiagPanel.
  diagRecordRender("FunctionBlock");

  if (!restComp || !structural) {
    // REST hasn't landed yet — render a placeholder. As soon as `components`
    // populates this uid, the Zustand selector re-renders us.
    return (
      <div
        style={{
          width: NODE_W,
          height: 40,
          background: "hsl(var(--card))",
          border: "1px dashed hsl(var(--input))",
          borderRadius: 4,
          color: "hsl(var(--muted-foreground))",
          fontSize: 11,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontFamily: "var(--font-mono)",
        }}
      >
        uid {data.componentUid}
      </div>
    );
  }
  const { rows, nodeH, kind, statusText, statusColor, statusPropExists, hiddenCount } = structural;

  return (
    <div
      onContextMenu={(e) => {
        // Resolve the row by walking up from the actual click target until we
        // hit an element tagged with data-row-uid. Both row divs AND their
        // Handle siblings carry the attribute, so a right-click anywhere on
        // the row (label, value, handle hit zone) lands on the same row.
        // Title bar has its own onContextMenu that stopPropagation()s, so
        // this only fires for body clicks.
        let el = e.target as Element | null;
        let uid: number | null = null;
        while (el && el !== e.currentTarget) {
          const v = (el as HTMLElement).dataset?.rowUid;
          if (v != null) {
            uid = Number(v);
            break;
          }
          el = el.parentElement;
        }
        if (uid == null) return;
        const p = rows.find((r) => r.uid === uid);
        if (!p) return;
        e.preventDefault();
        e.stopPropagation();
        // Read the live value/status imperatively at click time (the body no
        // longer subscribes to the stream).
        const flags =
          useStatusFlags.getState().flags.get(p.uid) ??
          restComp.properties[p.name]?.statusFlags ??
          0;
        setMenu({
          x: e.clientX,
          y: e.clientY,
          propName: p.name,
          propUid: p.uid,
          category: p.category,
          dataType: p.dataType,
          currentValue: useValues.getState().values.get(p.uid),
          overridden: (flags & STATUS_OVERRIDDEN) !== 0,
          exposed: !!p.exposed,
          exposedComponent: p.exposedComponent,
          portOwner: p.exposed ? data.componentUid : undefined,
        });
      }}
      style={{
        width: NODE_W,
        minHeight: nodeH,
        background: "hsl(var(--card))",
        border: selected
          ? "1px solid hsl(var(--cool))"
          : otherSelectors.length > 0
            ? `1px solid ${otherSelectors[0].color}`
            : "1px solid hsl(var(--border))",
        borderRadius: 4,
        color: "hsl(var(--foreground))",
        fontSize: 11,
        // Selection glow priority: our own selection (blue) wins; otherwise a
        // collaborator's selection paints a glow in their color. Both stack
        // their shadow over the default drop shadow.
        boxShadow: selected
          ? "0 0 0 1px hsl(var(--cool)), 0 0 12px rgba(74,158,255,0.45)"
          : otherSelectors.length > 0
            ? `0 0 0 1px ${otherSelectors[0].color}, 0 0 10px ${otherSelectors[0].color}66`
            : "0 1px 2px rgba(0,0,0,0.4)",
        transition: "box-shadow 80ms ease, border-color 80ms ease",
        position: "relative",
        overflow: "visible",
      }}
    >
      {otherSelectors.length > 0 && (
        <div
          style={{
            position: "absolute",
            top: -9,
            left: 6,
            display: "flex",
            gap: 3,
            zIndex: 5,
            pointerEvents: "none",
          }}
        >
          {otherSelectors.map((o) => (
            <span
              key={o.name}
              title={`${o.name} has this selected`}
              style={{
                fontSize: 9,
                lineHeight: "12px",
                padding: "0 4px",
                background: o.color,
                color: "hsl(var(--background))",
                borderRadius: 2,
                fontWeight: 600,
                fontFamily: "var(--font-mono)",
                whiteSpace: "nowrap",
              }}
            >
              {o.name}
            </span>
          ))}
        </div>
      )}
      <div
        onContextMenu={(e) => {
          // Node-level menu fires only when the user right-clicked the TITLE
          // BAR. The body below has its own per-row context menus (property
          // overrides / Connect to…) and right-clicking blank space between
          // rows should do nothing — keeps the body's right-click reserved
          // for property-targeted actions.
          if (!data.onContextMenu) return;
          e.preventDefault();
          e.stopPropagation();
          data.onContextMenu(data.componentUid, e.clientX, e.clientY);
        }}
        // Double-click the title to drill into the component's level (every
        // component can contain children, even if empty). Only here + the lip,
        // not the value rows (which use single-click to edit).
        onDoubleClick={(e) => {
          e.stopPropagation();
          data.onEnter?.(data.componentUid);
        }}
        style={{
          height: TITLE_H,
          padding: "4px 8px",
          background: "hsl(var(--secondary))",
          borderBottom: "1px solid hsl(var(--border))",
          display: "flex",
          flexDirection: "column",
          justifyContent: "center",
          boxSizing: "border-box",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
          <span
            style={{
              fontWeight: 600,
              fontSize: 12,
              flex: 1,
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {data.name ?? kind}
          </span>
          {statusPropExists && <StatusDot color={statusColor.bg} text={statusText} />}
        </div>
        <div
          style={{
            fontSize: 10,
            lineHeight: 1.35,
            color: "hsl(var(--muted-foreground))",
            fontFamily: "var(--font-mono)",
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
          title={kind}
        >
          {kind}
        </div>
      </div>

      {/* Bottom lip — a prop-height footer below the last row. Holds the
          drill-in (↵) button when this component has children, and a ⚡ marker
          when its type has actions. Hidden in LOD like the rows. */}
      {!lod && (
        <div
          onDoubleClick={(e) => {
            e.stopPropagation();
            data.onEnter?.(data.componentUid);
          }}
          title="Double-click to enter this component's level"
          style={{
            position: "absolute",
            left: 0,
            right: 0,
            top: TITLE_H + rows.length * ROW_H,
            height: ROW_H,
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "0 8px",
            boxSizing: "border-box",
            borderTop: "1px solid hsl(var(--border))",
            background: "hsl(var(--secondary))",
            borderBottomLeftRadius: 4,
            borderBottomRightRadius: 4,
            cursor: "pointer",
          }}
        >
          {/* left: actions marker + hidden-props indicator */}
          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
            {data.hasActions && (
              <span
                title="This component has actions"
                style={{ display: "flex", alignItems: "center", color: "hsl(var(--amber))" }}
              >
                <Zap size={12} strokeWidth={2} />
              </span>
            )}
            {hiddenCount > 0 && (
              <span
                title={`${hiddenCount} hidden propert${hiddenCount === 1 ? "y" : "ies"}`}
                style={{ display: "flex", alignItems: "center", color: "hsl(var(--muted-foreground))" }}
              >
                <EyeOff size={12} strokeWidth={2} />
              </span>
            )}
          </span>
          {/* right: has-children marker (double-click the block to enter) */}
          {data.hasChildren && (
            <span
              title={`Has ${data.childCount ?? ""} child${
                data.childCount === 1 ? "" : "ren"
              } — double-click to enter`}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 3,
                fontSize: 11,
                color: "hsl(var(--cool))",
                fontFamily: "var(--font-mono)",
              }}
            >
              <Layers size={12} strokeWidth={2} /> {data.childCount ?? ""}
            </span>
          )}
        </div>
      )}

      {/* Row CONTENT (labels + value cells) only at normal zoom. In LOD the
          node is just its title bar + handles — values aren't legible anyway,
          and skipping these divs is the bulk of the zoomed-out render saving. */}
      {!lod &&
        rows.map((p, i) => (
          <ValueRow
            key={p.uid}
            row={p}
            i={i}
            componentUid={data.componentUid}
            initialFlags={restComp.properties[p.name]?.statusFlags ?? 0}
          />
        ))}

      {rows.map((p, i) => {
        if (p.category === CATEGORY_CONFIG) return null;
        const isInput = p.category === CATEGORY_INPUT;
        const c = colorForType(p.dataType);
        // Handle hit zone, full row height. The visible 8px marker straddles the
        // node edge (half outside), so the hit box extends HANDLE_OUT past the edge
        // to cover the whole marker — otherwise only its inner half would be
        // clickable. React Flow anchors a FINISHED edge at the box's position-side
        // edge (right for outputs / left for inputs); keeping that == the marker's
        // outer tip means the edge meets the square with no gap. The drag line
        // anchors at the box center, only a few px inward. connectionRadius on the
        // canvas provides the generous TARGET snap.
        const HANDLE_OUT = 4; // px the hit box extends outside the node edge (= marker half-width)
        const HANDLE_W = 18;  // HANDLE_OUT outside + the rest inward
        const rowTop = TITLE_H + i * ROW_H;
        return (
          <Handle
            key={`h-${p.uid}`}
            id={String(p.uid)}
            type={isInput ? "target" : "source"}
            position={isInput ? Position.Left : Position.Right}
            // Same data-row-uid attribute as the row div — the root's
            // onContextMenu walks up from e.target to find this, so
            // right-clicks land on the right row regardless of whether
            // the cursor was over the row div, label, value, or the
            // handle hit zone overlay.
            data-row-uid={p.uid}
            style={{
              top: rowTop,
              [isInput ? "left" : "right"]: -HANDLE_OUT,
              width: HANDLE_W,
              height: ROW_H,
              background: "transparent",
              border: "none",
              borderRadius: 0,
              // Cancel React Flow's default translate; position is set by left/right.
              transform: "none",
            }}
          >
            <span
              style={{
                position: "absolute",
                top: "50%",
                // Marker centered on the node boundary (HANDLE_OUT in from the box
                // edge, then nudged out half its width).
                [isInput ? "left" : "right"]: HANDLE_OUT,
                transform: `translate(${isInput ? "-50%" : "50%"}, -50%)`,
                width: 8,
                height: 8,
                background: c,
                border: "1px solid hsl(var(--background))",
                borderRadius: 1,
                pointerEvents: "none",
              }}
            />
          </Handle>
        );
      })}
      {menu && (
        <PropertyContextMenu
          x={menu.x}
          y={menu.y}
          propName={menu.propName}
          propUid={menu.propUid}
          category={menu.category}
          dataType={menu.dataType}
          currentValue={menu.currentValue}
          overridden={menu.overridden}
          exposed={menu.exposed}
          portOwner={menu.portOwner}
          componentUid={menu.exposedComponent ?? data.componentUid}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}

export const FunctionBlock = memo(FunctionBlockInner, (a, b) => {
  return (
    a.selected === b.selected &&
    a.data.componentUid === b.data.componentUid &&
    a.data.name === b.data.name &&
    a.data.hasChildren === b.data.hasChildren &&
    a.data.childCount === b.data.childCount &&
    a.data.onEnter === b.data.onEnter &&
    a.data.onContextMenu === b.data.onContextMenu
  );
});

// --- Ghost node ---
// A sub-node placeholder for the off-canvas endpoint of a cross-folder edge.
// One row tall (lines up flush with the connected property on the real
// component), shows the external component's path + prop name, double-click
// jumps the breadcrumb to that component's folder.
//
// Has exactly one handle (`target` if it represents an external INPUT being
// fed by a visible output; `source` if it represents an external OUTPUT
// feeding a visible input). The handle id is the external property's uid so
// the cross-folder edge connects cleanly through React Flow's normal handle
// routing.

export interface GhostConnection {
  externalComponentUid: number;
  externalPath: string;     // e.g. "root/Services/foo"
  externalPropName: string;
  // Edge uid backing this connection — lets the popover delete a specific
  // edge from a fan-out without having to disambiguate via paths.
  edgeUid: number;
}

export type GhostNodeData = {
  // One ghost represents ONE handle on the visible component (one row on the
  // visible component) and aggregates ALL cross-folder edges that share that
  // endpoint. An input ghost has exactly one connection (inputs take at most
  // one incoming edge); an output ghost can have many (outputs fan out).
  // Without aggregation, multiple output-side ghosts would render at the
  // same Y and visually overlap into a single illegible blob.
  connections: GhostConnection[];
  // Shared handle id. All cross-folder edges that share this ghost reference
  // this id as source/targetHandle so they all converge on the same point.
  handleId: string;
  // Which side carries the handle.
  //   "input"  → ghost is the TARGET of an edge from a visible output, so its
  //              handle is on the LEFT (incoming). Connections list the
  //              external INPUT(s) being fed.
  //   "output" → ghost is the SOURCE of an edge into a visible input, so its
  //              handle is on the RIGHT (outgoing). Connections list the
  //              external OUTPUT(s) feeding it (almost always just one, but
  //              we use the same shape for symmetry).
  side: "input" | "output";
  // The visible component this ghost is anchored to, plus the row index of
  // the connected property — together these let App.tsx recompute the
  // ghost's position when the anchor component is dragged, so the ghost
  // follows along instead of being left behind.
  anchorUid: number;
  anchorRowIdx: number;
  // Width of THIS specific ghost, sized to its content by ghostWidthFor().
  // Stored on the data so the drag-along recomputation in App.tsx can place
  // left-side ghosts (output-source case) flush against the anchor's left
  // edge without recomputing the text length.
  width: number;
  // Navigate to a specific external component's folder (push crumbs).
  onNavigate?: (uid: number) => void;
  // Delete a specific edge backing one of this ghost's connections. The
  // ghost auto-disappears once its connections list empties — App.tsx
  // removes the ghost node when the connection count hits zero.
  onDeleteEdge?: (edgeUid: number) => void | Promise<void>;
};

function GhostNodeInner({ data }: { data: GhostNodeData }) {
  const isInputSide = data.side === "input";
  const [popOpen, setPopOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  const count = data.connections.length;
  // The label we show in the collapsed pill. Path shows the WHOLE path
  // including the component's own name (so "Services/RestService", not just
  // the folder "Services"), with the leading "root/" stripped — every path
  // starts with root, so the prefix is noise.
  const first = data.connections[0];
  // Collapsed pill shows just the component's own name; the full path is on the
  // hover title and in the click popover.
  const labelLeft = lastSegment(first?.externalPath ?? "");
  const labelRight = first?.externalPropName ?? "";

  const onClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    // Always open the popover. For N=1 it's a single-row "click to navigate"
    // affordance; for N>1 it lists all targets. Either way the interaction
    // is uniform — no special-case double-click semantics to remember.
    setPopOpen((v) => !v);
  };

  // Dismiss popover on outside click.
  useEffect(() => {
    if (!popOpen) return;
    const dismiss = (ev: MouseEvent) => {
      const el = ev.target as Element | null;
      if (el && el.closest("[data-ce-ghost-pop]")) return;
      if (el && rootRef.current?.contains(el)) return;
      setPopOpen(false);
    };
    document.addEventListener("mousedown", dismiss);
    return () => document.removeEventListener("mousedown", dismiss);
  }, [popOpen]);

  return (
    <div
      ref={rootRef}
      onClick={onClick}
      onDoubleClick={(e) => {
        e.stopPropagation();
        // For a single-connection ghost, double-click still navigates
        // directly — same affordance as before. For multi, double-click is
        // ambiguous so we just open the popover (the single-click above
        // already opened it; the second click would close it, so we
        // re-open here for consistency).
        if (count === 1) {
          data.onNavigate?.(data.connections[0].externalComponentUid);
          setPopOpen(false);
        } else {
          setPopOpen(true);
        }
      }}
      title={
        count === 1
          ? `${first?.externalPath} · ${first?.externalPropName} — double-click to open`
          : `${count} cross-folder connections — click to expand`
      }
      style={{
        // Inner box fills whatever width the RF node was given. App.tsx sizes
        // each ghost to its content so this collapses tight around the text.
        width: "100%",
        height: GHOST_H,
        background: popOpen ? "hsl(var(--card))" : "hsl(var(--background))",
        border: "1px dashed hsl(var(--muted-foreground))",
        borderRadius: 3,
        display: "flex",
        alignItems: "center",
        padding: "0 8px",
        gap: 6,
        fontSize: 10,
        fontFamily: "var(--font-mono)",
        color: "hsl(var(--muted-foreground))",
        whiteSpace: "nowrap",
        overflow: "hidden",
        cursor: "pointer",
        boxSizing: "border-box",
        // Make sure clicks reach us even though RF marks the node
        // non-selectable + non-draggable.
        pointerEvents: "all",
      }}
    >
      <span
        style={{
          color: "hsl(var(--cool))",
          overflow: "hidden",
          textOverflow: "ellipsis",
          minWidth: 0,
        }}
      >
        {labelLeft}
      </span>
      <span style={{ color: "hsl(var(--muted-foreground))", flexShrink: 0 }}>·</span>
      <span
        style={{
          color: "hsl(var(--foreground))",
          overflow: "hidden",
          textOverflow: "ellipsis",
          minWidth: 0,
        }}
      >
        {labelRight}
      </span>
      {count > 1 && (
        <span
          style={{
            flexShrink: 0,
            fontSize: 9,
            padding: "0 4px",
            background: "hsl(var(--cool))",
            color: "#fff",
            borderRadius: 2,
            fontWeight: 600,
          }}
        >
          +{count - 1}
        </span>
      )}
      <Handle
        id={data.handleId}
        type={isInputSide ? "target" : "source"}
        position={isInputSide ? Position.Left : Position.Right}
        style={{
          width: 8,
          height: 8,
          background: "hsl(var(--muted-foreground))",
          border: "1px solid hsl(var(--background))",
          borderRadius: 1,
          // Cancel React Flow's default 50% translate so the marker sits flush
          // at the ghost's edge, mirroring the real node's handle geometry.
          transform: "none",
          top: "50%",
          marginTop: -4,
          [isInputSide ? "left" : "right"]: -4,
        }}
      />
      {popOpen && rootRef.current && (
        <GhostPopover
          anchor={rootRef.current}
          isInputSide={isInputSide}
          connections={data.connections}
          onPick={(uid) => {
            setPopOpen(false);
            data.onNavigate?.(uid);
          }}
          onDeleteEdge={data.onDeleteEdge}
        />
      )}
    </div>
  );
}

// Renders below or beside the ghost when multiple cross-folder connections
// share the same handle. Each row is a navigation target on click, plus a ✕
// to delete that specific edge.
function GhostPopover({
  anchor,
  isInputSide,
  connections,
  onPick,
  onDeleteEdge,
}: {
  anchor: HTMLElement;
  isInputSide: boolean;
  connections: GhostConnection[];
  onPick: (externalUid: number) => void;
  onDeleteEdge?: (edgeUid: number) => void | Promise<void>;
}) {
  // Anchor below the ghost, aligned to its appropriate side. Using the
  // anchor's bounding rect (which is in viewport coords after RF's
  // transform) gives us correct placement at any zoom/pan.
  const rect = anchor.getBoundingClientRect();
  const top = rect.bottom + 4;
  const left = isInputSide ? rect.left : rect.right - 220;
  return createPortal(
    <div
      data-ce-ghost-pop
      onClick={(e) => e.stopPropagation()}
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        top,
        left,
        zIndex: 100,
        background: "hsl(var(--card))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 4,
        padding: 4,
        minWidth: 220,
        maxWidth: 360,
        maxHeight: 280,
        overflowY: "auto",
        boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
        fontSize: 11,
        color: "hsl(var(--foreground))",
        fontFamily: "var(--font-mono)",
      }}
    >
      <div
        style={{
          padding: "4px 8px 6px 8px",
          color: "hsl(var(--muted-foreground))",
          fontSize: 9,
          textTransform: "uppercase",
          letterSpacing: 0.4,
          borderBottom: "1px solid hsl(var(--border))",
          marginBottom: 4,
        }}
      >
        {connections.length} connection{connections.length === 1 ? "" : "s"}
      </div>
      {connections.map((c) => {
        const pathLabel = stripRoot(c.externalPath);
        return (
          <div
            key={c.edgeUid}
            style={{
              display: "flex",
              width: "100%",
              alignItems: "center",
              gap: 4,
              borderRadius: 2,
            }}
            onMouseEnter={(e) => (e.currentTarget.style.background = "hsl(var(--border))")}
            onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
          >
            <button
              onClick={() => onPick(c.externalComponentUid)}
              style={{
                display: "flex",
                flex: 1,
                minWidth: 0,
                alignItems: "baseline",
                gap: 6,
                padding: "4px 8px",
                background: "transparent",
                border: "none",
                color: "hsl(var(--foreground))",
                fontSize: 11,
                fontFamily: "inherit",
                cursor: "pointer",
                textAlign: "left",
              }}
              title="open this component's folder"
            >
              <span style={{ color: "hsl(var(--cool))", flexShrink: 0 }}>{pathLabel}</span>
              <span style={{ color: "hsl(var(--muted-foreground))" }}>·</span>
              <span style={{ color: "hsl(var(--foreground))" }}>{c.externalPropName}</span>
            </button>
            {onDeleteEdge && (
              <button
                onClick={() => void onDeleteEdge(c.edgeUid)}
                title="delete this edge"
                style={{
                  flexShrink: 0,
                  padding: "2px 6px",
                  marginRight: 4,
                  background: "transparent",
                  border: "1px solid transparent",
                  borderRadius: 2,
                  color: "hsl(var(--muted-foreground))",
                  cursor: "pointer",
                  fontFamily: "inherit",
                  fontSize: 11,
                }}
                onMouseEnter={(e) => {
                  e.currentTarget.style.background = "hsl(var(--crit) / 0.18)";
                  e.currentTarget.style.color = "hsl(var(--crit))";
                  e.currentTarget.style.borderColor = "hsl(var(--crit) / 0.25)";
                }}
                onMouseLeave={(e) => {
                  e.currentTarget.style.background = "transparent";
                  e.currentTarget.style.color = "hsl(var(--muted-foreground))";
                  e.currentTarget.style.borderColor = "transparent";
                }}
              >
                ✕
              </button>
            )}
          </div>
        );
      })}
    </div>,
    wiresheetPortalRoot(),
  );
}

export const GhostNode = memo(GhostNodeInner, (a, b) => {
  // Connections compared by reference + length; App.tsx rebuilds the array
  // on each reload so reference equality holds across unrelated updates.
  return (
    a.data.connections === b.data.connections &&
    a.data.handleId === b.data.handleId &&
    a.data.side === b.data.side &&
    a.data.width === b.data.width &&
    a.data.onNavigate === b.data.onNavigate
  );
});

// Helper for App.tsx — replicates FunctionBlock's row sort so a ghost can be
// positioned exactly at the Y of the connected property row in the visible
// component. Returns the row index, or -1 if the property isn't user-facing
// (system / non-normal systemRole — wouldn't be in any visible row).
export function userFacingRowIndex(comp: Component, propName: string): number {
  const entries = Object.entries(comp.properties).filter(
    ([, p]) => (p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL,
  );
  const order: PropertyCategory[] = [CATEGORY_OUTPUT, CATEGORY_INPUT, CATEGORY_CONFIG];
  const sorted: string[] = [];
  for (const cat of order) {
    for (const [n, p] of entries) {
      if (p.category === cat) sorted.push(n);
    }
  }
  return sorted.indexOf(propName);
}

// Row index of ANY rendered row — a user-facing prop OR an exposed PORT — by its
// prop uid. Unlike userFacingRowIndex this interleaves ports into their category
// groups exactly as `structural` does (outputs, then inputs, then config; each by
// facet `order`; hidden rows dropped unless linked), so a ghost on a folder/node
// port row lands at the right Y even when the node ALSO has user rows above it.
// Keep in lockstep with the `structural` row sort above. Returns -1 if not found.
export function rowIndexOf(
  comp: Component,
  facet: ComponentFacet,
  propUid: number,
  linked: Set<number>,
): number {
  interface R { uid: number; category: PropertyCategory; order?: number }
  const rows: R[] = [];
  for (const [, p] of Object.entries(comp.properties)) {
    if ((p.systemRole ?? ROLE_NORMAL) !== ROLE_NORMAL) continue;
    const f = facet.get(p.uid);
    if (f?.hidden && !linked.has(p.uid)) continue; // wired rows stay visible
    rows.push({ uid: p.uid, category: p.category, order: f?.order });
  }
  for (const ep of exposedPorts(facet)) {
    rows.push({
      uid: ep.childUid,
      category: ep.side === "input" ? CATEGORY_INPUT : CATEGORY_OUTPUT,
      order: ep.facet.order,
    });
  }
  const byOrder = (a: R, b: R) =>
    (a.order ?? Number.MAX_SAFE_INTEGER) - (b.order ?? Number.MAX_SAFE_INTEGER);
  const ordered = [
    ...rows.filter((r) => r.category === CATEGORY_OUTPUT).sort(byOrder),
    ...rows.filter((r) => r.category === CATEGORY_INPUT).sort(byOrder),
    ...rows.filter((r) => r.category === CATEGORY_CONFIG).sort(byOrder),
  ];
  return ordered.findIndex((r) => r.uid === propUid);
}
