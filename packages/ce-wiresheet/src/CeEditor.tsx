import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  MiniMap,
  PanOnScrollMode,
  SelectionMode,
  applyEdgeChanges,
  applyNodeChanges,
  useReactFlow,
  useStore as useRfStore,
  type Connection,
  type Edge as RfEdge,
  type EdgeChange,
  type Node as RfNode,
  type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { X, Maximize2, Minimize2, Network, PanelLeft } from "lucide-react";
import { UiTabHost } from "./ui/UiTabHost";
import { LeftPalette } from "./components/LeftPalette";
import { ExtensionStrip } from "./ui/ExtensionStrip";
import { getExtensions } from "./lib/ui/root-ext-stub";
import { findComponentUx } from "./lib/ui/uxLookup";
import type { ExtensionUi } from "./lib/ui/types";
import { wiresheetPortalRoot, setWiresheetPortalTheme } from "./lib/portal";
import "./wiresheet-theme.css"; // the editor's self-contained, host-overridable palette
import { createPortal } from "react-dom";

// Selected-edge highlight. RF adds .selected to .react-flow__edge when the
// edge's `selected` flag is true; the default stylesheet's selected color is
// hard to see on a dark canvas, so we paint a brighter stroke + drop shadow.
const EDGE_SELECTED_CSS = `
  .react-flow__edge.selected .react-flow__edge-path {
    stroke: hsl(var(--amber)) !important;
    stroke-width: 2.5 !important;
    filter: drop-shadow(0 0 4px rgba(255,209,102,0.6));
  }
`;

import { ClickDebugger } from "./components/ClickDebugger";
import { DiagDrawer } from "./components/DiagDrawer";
import { ConnectionStatus } from "./components/ConnectionStatus";
import { FindPanel } from "./components/FindPanel";
import { PresenceBar } from "./components/PresenceBar";
import { ZoomRateController } from "./components/ZoomRateController";
import { VisibilitySub } from "./components/VisibilitySub";
import {
  CeWiresheetContext,
  FunctionBlock,
  GHOST_H,
  GhostNode,
  NODE_W,
  ghostWidthFor,
  lastSegment,
  rowIndexOf,
  type FunctionBlockData,
  type GhostNodeData,
} from "./components/FunctionBlock";
import {
  addEdge as restAddEdge,
  addNode as restAddNode,
  bulkDelete,
  bulkUpdate,
  callAction,
  copyNodes,
  getNodeByUid,
  getRootNodes,
  getSchema,
  getSubtreeEdges,
  groupComponents,
  exposePort,
  unexposePort,
  patchOverrides,
  removeEdge as restRemoveEdge,
  removeNode as restRemoveNode,
  setEngineBase,
  setRestSessionId,
  setRestActorId,
  setRestGestureId,
  setRestTransport,
  newGestureId,
  withGesture,
  undoChange,
  redoChange,
  updateEdge as restUpdateEdge,
  updateNode,
  RestError,
} from "./lib/rest";
import type { EngineTransport, StreamHandlers, EngineStream } from "./lib/transport";
import { DirectTransport, type DirectEngineStream } from "./lib/transport-direct";
import type { Component, Edge, FlexValue } from "./lib/engine-types";
import {
  TYPE_STATUS,
  ROLE_NORMAL,
} from "./lib/engine-types";
import {
  loadSchemaIndices,
  propertyToComponent,
  useStatusFlags,
  useStructural,
  useValues,
} from "./lib/store";
import { loadSchemaChoices } from "./lib/choices";
import { metrics } from "./lib/instrumentation";
import {
  diagGauges,
  startDiagnostics,
  startDiagReporter,
  stopDiagnostics,
  stopDiagReporter,
} from "./lib/diagnostics";
import { usePresence, type PresenceState } from "./lib/presence";
import {
  facetFor,
  rawFacet,
  exposedPorts,
  FACET_PROP,
} from "./lib/facet";
import { sanitizeName, uniqueName } from "./lib/naming";
import { buildRfNodes, buildRfEdges, miniMapNodeColor, miniMapNodeStroke } from "./lib/rfbuild";
import { ErrorBanner } from "./editor/ErrorBanner";
import { EdgeContextMenu } from "./editor/menus/EdgeContextMenu";
import { PaneContextMenu } from "./editor/menus/PaneContextMenu";
import { NodeContextMenu } from "./editor/menus/NodeContextMenu";
import { MoveIntoPicker } from "./editor/pickers/MoveIntoPicker";
import { ActionPicker } from "./editor/pickers/ActionPicker";
import type { ActionDef } from "./editor/pickers/actions";
import { ConfigurePanel } from "./editor/panels/ConfigurePanel";
import type { PaletteExtension } from "./editor/menus/types";
import { partitionEdges, exposedPortIndex, classifyCrossEdge } from "./lib/routing";
import { planPaste } from "./lib/paste";

const nodeTypes = { fb: FunctionBlock, ghost: GhostNode };

// Constants for ghost layout. ROW_H lives in FunctionBlock; we keep the title
// height local because it's only used here for ghost-Y math (it's also defined
// there for the node itself).
const FB_TITLE_H = 40;
const FB_ROW_H = 18;
// Horizontal gap between a visible component and its ghost sub-node. Kept tight
// so the ghost reads as belonging to its property row rather than floating off.
const GHOST_GAP = 16;

// Module-level stream singleton so HMR / StrictMode double mounts don't open two
// sockets. Holds the active EngineStream (whatever transport minted it) — the
// rate/subscription controls and the presence layer reach the live stream
// through `streamRef.current`.
const streamRef: { current: EngineStream | null } = { current: null };

// MIME type for drag-and-drop from the palette into the React Flow canvas. Custom so
// we don't conflict with any other drop sources (text, files, etc).
const DND_TYPE = "application/x-ce-component-type";



// Root component UID. The engine's root is always 0.
const ROOT_UID = 0;

// Tiny stable string hash (djb2) — used to derive a per-tab actor id from the
// session id so the engine scopes this editor's undo/redo stack to itself.
function djb2(s: string): number {
  let h = 5381;
  for (let i = 0; i < s.length; i++) h = ((h << 5) + h + s.charCodeAt(i)) | 0;
  return h >>> 0;
}

// Pointer travel (px) that turns a right-press into a marquee drag rather than a
// click. Generous so a normal click's jitter never registers as a drag — a real
// drag-select moves much further. The marquee select and the contextmenu
// suppression key off the SAME activation, so they never both fire.
const MARQUEE_DRAG_PX = 8;

// Per-tab discriminator for the presence display name. Generated once at module
// load — unique per tab (a duplicated tab re-runs module init, so it differs
// even though it copied storage). Short, derived from the high-res clock.
const TAB_SUFFIX = Math.trunc(performance.now() * 1000 + performance.timeOrigin)
  .toString(36)
  .slice(-4);

// `base` is the selected control engine's REST origin, e.g.
// `http://192.168.1.50:7878`. The standalone harness passes its own (proxied)
// origin; the extension passes the selected device's `ip:port`.
//
// `transport` is the optional EngineTransport seam (see lib/transport.ts):
// omit it and the editor talks straight to `base` exactly as it always has
// (`new DirectTransport()`, wired to `setEngineBase(base)`). A host that
// injects its own transport (e.g. an MCP/Zenoh bridge) is responsible for
// routing requests to the right engine itself — `base` is then only used for
// display/labeling by such a host, not dialed directly by this component.
export default function CeEditor({
  base,
  transport,
}: {
  base: string;
  transport?: EngineTransport;
}) {
  return (
    <ReactFlowProvider>
      <Inner base={base} transport={transport ?? new DirectTransport()} />
    </ReactFlowProvider>
  );
}

interface Crumb {
  uid: number;
  name: string;
}

// React Flow stamps its `colorMode` onto the flow container as a class
// (`react-flow dark` / `react-flow light`). rbx's theme.css keys its palette off
// those SAME class names (`.dark` / `.light`), so if React Flow's default
// (`light`) disagrees with the document's theme, it re-activates the wrong
// palette inside the canvas — e.g. white `--card` nodes while `<html>` is dark.
// Mirror the document's actual theme class onto colorMode so they always agree.
function useDocumentColorMode(): "dark" | "light" {
  const read = (): "dark" | "light" =>
    typeof document !== "undefined" && document.documentElement.classList.contains("light")
      ? "light"
      : "dark"; // default (`:root`) is the dark palette
  const [mode, setMode] = useState<"dark" | "light">(read);
  useEffect(() => {
    const el = document.documentElement;
    const obs = new MutationObserver(() => setMode(read()));
    obs.observe(el, { attributes: true, attributeFilter: ["class"] });
    setMode(read());
    return () => obs.disconnect();
  }, []);
  return mode;
}

// Orthogonal edge routing: right angles with rounded corners instead of bezier
// curves, so wires hug horizontal/vertical lanes and read more cleanly (they still
// don't actively avoid nodes — that needs pathfinding). Applied to every edge we
// build; React Flow's `defaultEdgeOptions.type` only covers onConnect-added edges,
// which we don't use.
const EDGE_TYPE = "smoothstep" as const;

// Canvas grid pitch (px). Shared by the background dots and node snap-to-grid so
// dragged components align to the visible grid.
const GRID_GAP = 20;

function Inner({ base, transport }: { base: string; transport: EngineTransport }) {
  const colorMode = useDocumentColorMode();
  // Keep the portaled-overlay container (menus, marquee — outside this subtree) on
  // the same light/dark as the editor so they share the scoped tokens.
  useEffect(() => setWiresheetPortalTheme(colorMode), [colorMode]);
  // Point the REST client at the selected engine before any request fires.
  // useMemo runs during render (ahead of the effects that call reload()), so
  // the first fetch already targets `<base>/api/v0`. setEngineBase configures
  // DirectTransport specifically (a no-op module-state write when a
  // non-direct transport is injected); setRestTransport is what actually
  // routes every rest.ts wrapper's request() through the given transport.
  useMemo(() => setEngineBase(base), [base]);
  useMemo(() => setRestTransport(transport), [transport]);

  // Nodes contain both real function blocks AND ghost sub-nodes for cross-folder
  // edge endpoints. RF's nodeTypes routes each to its renderer by `type`.
  type AnyNode = RfNode<FunctionBlockData> | RfNode<GhostNodeData>;
  const [nodes, setNodes] = useState<AnyNode[]>([]);
  const [edges, setEdges] = useState<RfEdge[]>([]);
  // Edges from the last reload, parked here until every node has been measured.
  // Without this gate React Flow tries to place edges before handles exist in its
  // internal lookup → "Couldn't create edge for source handle id …" warning.
  const [pendingEdges, setPendingEdges] = useState<RfEdge[] | null>(null);
  // Concurrent-reload guard. A reload applies UNLESS a strictly NEWER reload has
  // ALREADY applied (so a stale topology-triggered reload — e.g. one that fetched
  // before a Group's facet write — can't clobber a fresher one). Keyed on
  // "already applied", NOT "already started", so a burst of reloads never starves
  // (the highest-gen one always applies; only superseded ones drop).
  const reloadGen = useRef(0);
  const lastAppliedReloadGen = useRef(0);

  // Our WS session id; used to distinguish own echo (instant snap) from remote
  // topology changes (animate). Set from the schema callback below.
  const sessionIdRef = useRef<string | null>(null);

  // Position-tween state for remote-origin position changes. Keyed by node id.
  //
  // We use a critically-damped exponential ease instead of a fixed-duration curve:
  // on each rAF tick the node's current position moves toward the target by a
  // fraction `1 - exp(-RATE * dt)`. Properties of this approach that fixed-duration
  // easeOut doesn't have:
  //   - Velocity-aware retargeting. A second position update arriving mid-flight
  //     doesn't restart from rest — the easing simply pulls toward the new target
  //     from wherever the node currently is. No double-tween hitch.
  //   - Frame-independent. Reads dt from the rAF clock, so a dropped frame just
  //     means the next tick takes a bigger step; no time wobble.
  //   - Settles asymptotically. We snap to target and drop the entry when both
  //     axes are within 0.5px.
  // RATE is per-second; higher = snappier settle. 9 ≈ ~400ms to within 1% of target.
  const POS_ANIM_RATE = 9;
  const POS_SETTLE_PX = 0.5;
  const posAnims = useRef(
    new Map<
      string,
      { curPos: { x: number; y: number }; endPos: { x: number; y: number } }
    >(),
  );
  const posAnimRaf = useRef<number | null>(null);
  const posAnimLastTick = useRef<number | null>(null);

  const tickPosAnims = useCallback(() => {
    const now = performance.now();
    const last = posAnimLastTick.current;
    // Clamp dt so an idle tab returning to focus doesn't make the next step
    // jump the whole remaining distance in one frame.
    const dt = last != null ? Math.min(0.05, (now - last) / 1000) : 1 / 60;
    posAnimLastTick.current = now;
    const anims = posAnims.current;
    if (anims.size === 0) {
      posAnimRaf.current = null;
      posAnimLastTick.current = null;
      return;
    }
    const alpha = 1 - Math.exp(-POS_ANIM_RATE * dt);
    const patch = new Map<string, { x: number; y: number }>();
    for (const [id, a] of anims) {
      const nx = a.curPos.x + (a.endPos.x - a.curPos.x) * alpha;
      const ny = a.curPos.y + (a.endPos.y - a.curPos.y) * alpha;
      if (
        Math.abs(a.endPos.x - nx) < POS_SETTLE_PX &&
        Math.abs(a.endPos.y - ny) < POS_SETTLE_PX
      ) {
        patch.set(id, a.endPos);
        anims.delete(id);
      } else {
        a.curPos = { x: nx, y: ny };
        patch.set(id, { x: nx, y: ny });
      }
    }
    if (patch.size > 0) {
      setNodes((ns) =>
        ns.map((n) => {
          // Anchor components — apply the tween position from the patch.
          const p = patch.get(n.id);
          if (p) return { ...n, position: p };
          // Ghosts anchored to a moving component follow along so the edge
          // stub stays glued during cross-window position animations.
          if (n.type === "ghost") {
            const g = n as RfNode<GhostNodeData>;
            const anchor = patch.get(String(g.data.anchorUid));
            if (!anchor) return n;
            const gx =
              g.data.side === "input"
                ? anchor.x + NODE_W + GHOST_GAP
                : anchor.x - g.data.width - GHOST_GAP;
            const gy = anchor.y + FB_TITLE_H + g.data.anchorRowIdx * FB_ROW_H;
            return { ...g, position: { x: gx, y: gy } };
          }
          return n;
        }),
      );
    }
    posAnimRaf.current = anims.size > 0 ? requestAnimationFrame(tickPosAnims) : null;
    if (anims.size === 0) posAnimLastTick.current = null;
  }, []);

  const animateNodeTo = useCallback(
    (id: string, fromPos: { x: number; y: number }, toPos: { x: number; y: number }) => {
      const existing = posAnims.current.get(id);
      // Retarget: keep `curPos` if we're already animating (preserves visual
      // continuity — the new pull starts from wherever the node currently is,
      // not from the call site's `fromPos` which lags React render).
      posAnims.current.set(id, {
        curPos: existing ? existing.curPos : fromPos,
        endPos: toPos,
      });
      if (posAnimRaf.current == null) {
        posAnimRaf.current = requestAnimationFrame(tickPosAnims);
      }
    },
    [tickPosAnims],
  );

  useEffect(() => {
    return () => {
      if (posAnimRaf.current != null) {
        cancelAnimationFrame(posAnimRaf.current);
        posAnimRaf.current = null;
      }
      posAnims.current.clear();
      posAnimLastTick.current = null;
    };
  }, []);

  const [error, setError] = useState<{ message: string; debug?: string } | null>(null);
  // Normalise any caught value into the banner shape; RestErrors carry a
  // copy-pasteable request/response dump for debugging.
  const reportError = useCallback((e: unknown) => {
    if (e instanceof RestError) setError({ message: e.message, debug: e.debug });
    else setError({ message: e instanceof Error ? e.message : String(e) });
  }, []);
  const [palette, setPalette] = useState<PaletteExtension[]>([]);
  // Action signatures indexed by component type, built from the same `/schema`
  // pass as the palette. Read on right-click — no per-open fetch.
  const [actionsByType, setActionsByType] = useState<Map<string, ActionDef[]>>(
    () => new Map(),
  );
  // Same info as a type-set in a ref, so `buildRfNodes` can stamp `hasActions`
  // without making `reload` depend on the (async-loaded) action index.
  const actionTypesRef = useRef<Set<string>>(new Set());
  const [crumbs, setCrumbs] = useState<Crumb[]>([{ uid: ROOT_UID, name: "root" }]);
  const currentParentUid = crumbs[crumbs.length - 1].uid;
  const rf = useReactFlow();

  // Mouse model: LEFT-drag on the pane pans (panOnDrag={[0]}); RIGHT-drag on the
  // pane is a marquee box-select. (React Flow's built-in selectionOnDrag is
  // left-button-only, so the right-button marquee is implemented here.)
  //
  // Direction convention (CAD-style): left-to-right marquee = "fully enclosed"
  // only; right-to-left = "touching" too. Maps to getIntersectingNodes's
  // `partially` flag.
  const marquee = useRef<{ startX: number; startY: number; active: boolean } | null>(null);
  const [marqueeRect, setMarqueeRect] = useState<
    { x: number; y: number; w: number; h: number } | null
  >(null);

  // Is this pointer event on the empty pane (not a node / edge / handle)? Only
  // then does a right-drag start a marquee — right-clicking a node/edge/row
  // still opens its context menu.
  const isPaneTarget = (target: EventTarget | null): boolean => {
    let el = target as Element | null;
    while (el) {
      if (el.classList?.contains("react-flow__node")) return false;
      if (el.classList?.contains("react-flow__edge")) return false;
      if (el.classList?.contains("react-flow__handle")) return false;
      if (el.classList?.contains("react-flow__pane")) return true;
      el = el.parentElement;
    }
    return false;
  };

  const onCanvasPointerDown = useCallback((e: React.PointerEvent) => {
    if (e.button === 2 && isPaneTarget(e.target)) {
      marquee.current = { startX: e.clientX, startY: e.clientY, active: false };
    }
  }, []);
  const onCanvasPointerMove = useCallback((e: React.PointerEvent) => {
    const m = marquee.current;
    if (!m) return;
    const dx = e.clientX - m.startX;
    const dy = e.clientY - m.startY;
    if (!m.active && Math.hypot(dx, dy) < MARQUEE_DRAG_PX) return; // not a drag yet
    m.active = true;
    setMarqueeRect({
      x: Math.min(m.startX, e.clientX),
      y: Math.min(m.startY, e.clientY),
      w: Math.abs(dx),
      h: Math.abs(dy),
    });
  }, []);
  const onCanvasPointerUp = useCallback(
    (e: React.PointerEvent) => {
      const m = marquee.current;
      marquee.current = null;
      if (!m) return; // not a right-pane gesture
      if (!m.active) {
        // Quick right-click (no drag) → open the pane menu HERE, on release.
        // The browser fires `contextmenu` on PRESS — before we know whether the
        // gesture becomes a drag-select — so the menu can't be opened there
        // without also firing on drags. Deciding at pointer-up fixes that.
        setMarqueeRect(null);
        setNodeMenu(null);
        setPaneMenu({ x: e.clientX, y: e.clientY });
        return;
      }
      // Drag → marquee selection. Screen rect → flow rect via the two corners.
      const a = rf.screenToFlowPosition({ x: m.startX, y: m.startY });
      const b = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
      const rect = {
        x: Math.min(a.x, b.x),
        y: Math.min(a.y, b.y),
        width: Math.abs(b.x - a.x),
        height: Math.abs(b.y - a.y),
      };
      // left-to-right drag → fully-enclosed only; right-to-left → touching.
      const partially = e.clientX < m.startX;
      const hits = rf.getIntersectingNodes(rect, partially);
      const hitIds = new Set(hits.filter((n) => n.type !== "ghost").map((n) => n.id));
      const multi = e.shiftKey || e.metaKey || e.ctrlKey;
      setNodes((ns) =>
        ns.map((n) => {
          if (n.type === "ghost") return n;
          const want = multi ? n.selected || hitIds.has(n.id) : hitIds.has(n.id);
          return n.selected === want ? n : { ...n, selected: want };
        }),
      );
      if (!multi) setEdges((es) => es.map((ed) => (ed.selected ? { ...ed, selected: false } : ed)));
      setMarqueeRect(null);
    },
    [rf],
  );

  // Document-level click selection. Runs in CAPTURE phase, before React Flow's own
  // handlers, so `selectionOnDrag` can't swallow the event. We track pointerdown +
  // pointerup positions to decide "click" (under 4px movement) and apply selection
  // ourselves. RF's own selection emits land on top harmlessly via setNodes equality.
  //
  // Two clickable targets are tracked: nodes (.react-flow__node) and edges
  // (.react-flow__edge). Edges resolve through their interaction overlay path —
  // RF renders a thick invisible <path class="react-flow__edge-interaction"> as a
  // hit zone on top of the visible stroke.
  useEffect(() => {
    type HitTarget =
      | { kind: "node"; id: string }
      | { kind: "edge"; id: string }
      | null;
    const findHit = (target: EventTarget | null): HitTarget => {
      let el = target as Element | null;
      while (el) {
        if (el.classList?.contains("react-flow__node")) {
          const id = (el as HTMLElement).dataset.id ?? null;
          return id ? { kind: "node", id } : null;
        }
        if (el.classList?.contains("react-flow__edge")) {
          const id = (el as HTMLElement).dataset.id ?? null;
          return id ? { kind: "edge", id } : null;
        }
        if (el.classList?.contains("react-flow__pane")) return null;
        el = el.parentElement;
      }
      return null;
    };
    const isPane = (target: EventTarget | null): boolean => {
      let el = target as Element | null;
      while (el) {
        if (el.classList?.contains("react-flow__pane")) return true;
        if (el.classList?.contains("react-flow__node")) return false;
        if (el.classList?.contains("react-flow__edge")) return false;
        el = el.parentElement;
      }
      return false;
    };
    let downAt: { x: number; y: number; hit: HitTarget } | null = null;
    const onDown = (e: PointerEvent) => {
      if (e.button !== 0) {
        downAt = null;
        return;
      }
      downAt = { x: e.clientX, y: e.clientY, hit: findHit(e.target) };
    };
    const onUp = (e: PointerEvent) => {
      const d = downAt;
      downAt = null;
      if (!d) return;
      const dist = Math.hypot(e.clientX - d.x, e.clientY - d.y);
      if (dist > 4) return; // a drag, not a click

      const upHit = findHit(e.target);
      const multi = e.shiftKey || e.metaKey || e.ctrlKey;

      // Click resolved on the same node — toggle selection.
      if (d.hit?.kind === "node" && upHit?.kind === "node" && upHit.id === d.hit.id) {
        const id = d.hit.id;
        // Ghosts handle their own clicks (popover). Leave them alone so the
        // doc-level handler doesn't flip their selection state under the
        // popover's feet, and so React's onClick on the ghost actually fires.
        if (id.startsWith("ghost:")) return;
        metrics.lastSelChange = `click→${useStructural.getState().components.get(Number(id))?.name ?? id} (capture)`;
        metrics.lastSelChangeAt = performance.now();
        setNodes((ns) =>
          ns.map((n) => {
            if (multi) return n.id === id ? { ...n, selected: !n.selected } : n;
            const want = n.id === id;
            return n.selected === want ? n : { ...n, selected: want };
          }),
        );
        if (!multi) {
          setEdges((es) => es.map((edge) => (edge.selected ? { ...edge, selected: false } : edge)));
        }
        return;
      }

      // Click resolved on the same edge — toggle selection.
      if (d.hit?.kind === "edge" && upHit?.kind === "edge" && upHit.id === d.hit.id) {
        const id = d.hit.id;
        metrics.lastSelChange = `edge→${id}`;
        metrics.lastSelChangeAt = performance.now();
        setEdges((es) =>
          es.map((edge) => {
            if (multi) return edge.id === id ? { ...edge, selected: !edge.selected } : edge;
            const want = edge.id === id;
            return edge.selected === want ? edge : { ...edge, selected: want };
          }),
        );
        if (!multi) {
          setNodes((ns) => ns.map((n) => (n.selected ? { ...n, selected: false } : n)));
        }
        return;
      }

      // Clean click on the pane → clear selection.
      if (!d.hit && !upHit && isPane(e.target)) {
        metrics.lastSelChange = "pane→clear (capture)";
        metrics.lastSelChangeAt = performance.now();
        setNodes((ns) => ns.map((n) => (n.selected ? { ...n, selected: false } : n)));
        setEdges((es) => es.map((edge) => (edge.selected ? { ...edge, selected: false } : edge)));
      }
    };
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("pointerup", onUp, true);
    return () => {
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("pointerup", onUp, true);
    };
  }, []);

  // Escape clears the current selection on both nodes and edges. React Flow doesn't
  // bind this by default. Listening at the window level so it works regardless of
  // focus state (clicking on a node usually doesn't move focus to the canvas).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      // If the focus is in an input/textarea (e.g. the palette filter), let Esc do
      // whatever the input wants instead of clearing canvas selection.
      const ae = document.activeElement;
      if (ae && (ae.tagName === "INPUT" || ae.tagName === "TEXTAREA" || (ae as HTMLElement).isContentEditable)) {
        return;
      }
      setNodes((ns) => ns.map((n) => (n.selected ? { ...n, selected: false } : n)));
      setEdges((es) => es.map((edge) => (edge.selected ? { ...edge, selected: false } : edge)));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Click-into handler — pushed into each block's `data.onEnter` so a memo equality
  // check on identity is stable across renders for a given crumb stack.
  const enter = useCallback((uid: number) => {
    const c = useStructural.getState().components.get(uid);
    if (!c) return;
    setCrumbs((cur) => [...cur, { uid: c.uid, name: c.name || c.type }]);
  }, []);

  // After a navigation jump (ghost double-click), the target component lives
  // in a different folder than the current view. We set this so the
  // post-reload effect below knows which node to center + select once the
  // new view's nodes are mounted.
  const [focusAfterLoad, setFocusAfterLoad] = useState<number | null>(null);
  // On a folder change (drill in / up / breadcrumb), once the new level's nodes
  // load: if the folder we just left is now a visible node (we went UP), center
  // on it; otherwise (we went DOWN / elsewhere) fit the whole level into view.
  const prevParentRef = useRef(currentParentUid);
  const [navAfterLoad, setNavAfterLoad] = useState<{ from: number } | null>(null);

  // Component finder (Cmd/Ctrl+F). Searches the whole tree, jumps to a pick.
  const [findOpen, setFindOpen] = useState(false);

  // Diagnostics + events bottom drawer. The ConnectionStatus handle in the
  // bottom bar toggles it; persisted so it survives reloads.
  const [diagOpen, setDiagOpen] = useState<boolean>(() => {
    try {
      return window.localStorage.getItem("ce-ui.diagdrawer.open") === "1";
    } catch {
      return false;
    }
  });
  useEffect(() => {
    try {
      window.localStorage.setItem("ce-ui.diagdrawer.open", diagOpen ? "1" : "0");
    } catch {
      /* ignore */
    }
  }, [diagOpen]);
  // Measured bottom-bar height, so the drawer sits exactly on top of it.
  const bottomBarRef = useRef<HTMLDivElement>(null);
  // Bottom bar removed (matches rbx); the diagnostics drawer now sits at the very
  // bottom (no bar to clear). The ref/observer below stay harmless (ref is null).
  const [bottomBarH, setBottomBarH] = useState(0);
  useEffect(() => {
    const el = bottomBarRef.current;
    if (!el) return;
    setBottomBarH(el.offsetHeight);
    const ro = new ResizeObserver(() => setBottomBarH(el.offsetHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Click-debug overlay (the selection-diagnostics rings + bottom-right log).
  // Off by default — it was for chasing the marquee/select bugs (now fixed) and
  // it sits over the minimap. Toggle with Cmd/Ctrl+Shift+D. Persisted.
  const [clickDebugOpen, setClickDebugOpen] = useState(() => {
    try {
      return window.localStorage.getItem("ce-ui.clickDebug") === "1";
    } catch {
      return false;
    }
  });
  useEffect(() => {
    try {
      window.localStorage.setItem("ce-ui.clickDebug", clickDebugOpen ? "1" : "0");
    } catch {
      /* ignore */
    }
  }, [clickDebugOpen]);

  // Newly-pasted component uids waiting to land in the next reload's nodes
  // array. The post-reload effect promotes them to the active selection
  // once they appear, then clears this so unrelated nodes updates don't
  // re-trigger.
  const [pendingPasteSelection, setPendingPasteSelection] = useState<number[] | null>(null);

  // Navigate to a component's containing folder. Used by ghost sub-nodes
  // (cross-folder edge endpoints) on double-click. The component may live in
  // a folder several levels up from the current view, so we walk the
  // ancestor chain via REST to build a full crumb stack (otherwise the
  // breadcrumb would lie about depth). One REST call per ancestor — fine for
  // a user-initiated jump.
  const goToComponent = useCallback(async (uid: number) => {
    try {
      const targetResp = await getNodeByUid(uid, { depth: 0 });
      const target = targetResp.nodes[0];
      if (!target) return;
      // Walk up from target.parent to root, recording {uid, name} at each
      // level. Stop at root (uid 0).
      const chain: Crumb[] = [];
      let cursor = target.parent;
      while (cursor !== ROOT_UID) {
        const r = await getNodeByUid(cursor, { depth: 0 });
        const c = r.nodes[0];
        if (!c) break;
        chain.unshift({ uid: c.uid, name: c.name || c.type });
        if (c.parent === c.uid) break; // defensive
        cursor = c.parent;
      }
      // Mark the target so the post-reload effect picks it up. Set BEFORE
      // setCrumbs so the effect, which depends on `nodes`, finds the focus
      // request already armed when the new node list lands.
      setFocusAfterLoad(uid);
      setCrumbs([{ uid: ROOT_UID, name: "root" }, ...chain]);
    } catch (e) {
      reportError(e);
    }
  }, []);

  // Node-level right-click context menu state. Opened by a FunctionBlock when
  // the user right-clicks the body (not a property row). Lives at App level so
  // the menu / picker can read the current multi-selection from `nodes`.
  const [nodeMenu, setNodeMenu] = useState<{ x: number; y: number; uid: number } | null>(
    null,
  );
  const [movePickerOpen, setMovePickerOpen] = useState(false);
  const [actionPickerOpen, setActionPickerOpen] = useState(false);
  const [detailsUid, setDetailsUid] = useState<number | null>(null);
  // Right-click on empty pane → menu (up a folder / add component / paste).
  const [paneMenu, setPaneMenu] = useState<{ x: number; y: number } | null>(null);
  const openNodeContextMenu = useCallback(
    (uid: number, x: number, y: number) => {
      setNodeMenu({ x, y, uid });
      setPaneMenu(null);
      // Fresh menu — never reopen straight into a sub-picker.
      setMovePickerOpen(false);
      setActionPickerOpen(false);
      // Right-click acts on the right-clicked node by default; if it's already
      // in a multi-selection we leave that selection intact. If it's NOT
      // selected, replace the selection with just this node so the action's
      // target is unambiguous.
      setNodes((ns) => {
        const target = ns.find((n) => n.id === String(uid));
        if (target?.selected) return ns;
        return ns.map((n) => {
          const want = n.id === String(uid);
          return n.selected === want ? n : { ...n, selected: want };
        });
      });
    },
    [],
  );

  // The action-discovery seam. TODAY: actions are type-level, so resolve each
  // selected component's type → its signatures from the cached schema, and
  // return the actions common to ALL targets (so the chosen action exists on
  // every one). LATER (per-instance/dynamic actions): filter/merge this by a
  // live `Map<uid, availableActionNames>` (fetched on open or WS-pushed) — only
  // this function changes; the picker and invoke path stay the same.
  const getActionsFor = useCallback(
    (uids: number[]): ActionDef[] => {
      const comps = useStructural.getState().components;
      const lists = uids
        .map((u) => comps.get(u)?.type)
        .filter((t): t is string => !!t)
        .map((t) => actionsByType.get(t) ?? []);
      if (lists.length === 0) return [];
      const [first, ...rest] = lists;
      return first.filter((a) => rest.every((l) => l.some((b) => b.name === a.name)));
    },
    [actionsByType],
  );

  // Dispatch one action on every target. Actions are per-component, so we fan
  // out one `POST /call` per uid (the bulk endpoints don't dispatch actions).
  const invokeAction = useCallback(
    (uids: number[], action: string, params: Record<string, FlexValue>) =>
      Promise.all(uids.map((u) => callAction(u, action, params))),
    [],
  );

  const goToCrumb = useCallback((idx: number) => {
    setCrumbs((cur) => cur.slice(0, idx + 1));
  }, []);

  // Copy = remember the selected component UIDs + their on-screen centroid.
  // Paste clones them server-side via POST /copy/nodes (full fidelity, internal
  // edges auto-included), so the clipboard is just the uid list — no
  // value/edge snapshotting. The centroid lets paste translate the clones so
  // their group lands at the cursor preserving relative layout.
  const copySelectionToClipboard = useCallback(() => {
    const selectedReal = nodes.filter((n) => n.selected && n.type !== "ghost");
    if (selectedReal.length === 0) return;
    const uids = selectedReal.map((n) => Number(n.id));
    const xs = selectedReal.map((n) => n.position.x);
    const ys = selectedReal.map((n) => n.position.y);
    const centroid = {
      x: (Math.min(...xs) + Math.max(...xs)) / 2,
      y: (Math.min(...ys) + Math.max(...ys)) / 2,
    };
    // Mouse position at copy (flow coords) — paste shifts by (pasteCursor - this)
    // so nodes keep their offset from where the mouse was when you copied.
    const cursor = rf.screenToFlowPosition(mouseScreenPos.current);
    clipboardRef.current = { uids, centroid, cursor };
    metrics.lastSelChange = `copied ${uids.length}c`;
    metrics.lastSelChangeAt = performance.now();
  }, [nodes, rf]);

  // (pasteFromClipboard and the Cmd+C/V keyboard listener are declared
  // below — they depend on `reload`, which is itself declared further down.)

  // Delete a single cross-folder edge backing one of a ghost's connections.
  // Same REST + local-state pattern as onEdgesDelete, plus shrinks the
  // ghost's connections list (or removes the ghost entirely when its last
  // connection goes — "ghost has nothing left to point at"). Declared above
  // reload() so reload() can wire it into ghost data on every rebuild.
  const deleteGhostEdge = useCallback(async (edgeUid: number) => {
    try {
      await restRemoveEdge(edgeUid);
    } catch (e) {
      reportError(e);
      return;
    }
    useStructural.getState().removeEdge(edgeUid);
    setEdges((es) => es.filter((e) => e.id !== String(edgeUid)));
    setNodes((ns) =>
      ns.flatMap((n) => {
        if (n.type !== "ghost") return [n];
        const g = n as RfNode<GhostNodeData>;
        const idx = g.data.connections.findIndex((c) => c.edgeUid === edgeUid);
        if (idx < 0) return [n];
        const next = g.data.connections.filter((_, i) => i !== idx);
        if (next.length === 0) return []; // last edge gone — drop the ghost
        return [{ ...g, data: { ...g.data, connections: next } }];
      }),
    );
  }, []);

  // All edges scoped to the current view (incl. cross-folder ones that reach
  // off-canvas) from the last reload. The store keeps only inEdges; grouping
  // needs the cross-folder edges too (to expose ports through folder members).
  const scopedEdgesRef = useRef<Edge[]>([]);

  // Load the children of the current parent. depth=1, nested=true gets the parent +
  // its immediate children with `childrenCount` populated. We render only the children
  // (not the parent itself), so the user is "inside" the parent's container.
  const reload = useCallback(async () => {
    const gen = ++reloadGen.current;
    try {
      let resp;
      if (currentParentUid === ROOT_UID) {
        resp = await getRootNodes({ depth: 1, nested: true, withEdges: true });
      } else {
        // withEdges: true makes /nodes/uid/X return every edge with at least
        // one endpoint inside this subtree — INCLUDING cross-folder edges
        // that reach outside. GET /edges?component=X scopes too tightly (it
        // only returns edges entirely within the subtree), so cross-folder
        // ghosts wouldn't render when viewing a child folder.
        resp = await getNodeByUid(currentParentUid, {
          depth: 1,
          nested: true,
          withEdges: true,
        });
      }
      // Drop only if a strictly newer reload already applied (stale clobber).
      if (gen < lastAppliedReloadGen.current) return;
      lastAppliedReloadGen.current = gen;
      const parent = resp.nodes[0];
      const children = parent?.children ?? [];
      let scopedEdges: Edge[] = resp.edges ?? [];
      const childUids = new Set(children.map((c) => c.uid));
      const childByUid = new Map(children.map((c) => [c.uid, c]));
      // When the visible children are FOLDERS that project ports, the edges those
      // ports carry live among GRANDCHILDREN — a depth-1 read returns none of them.
      // Fetch the classified subtree-edge view (API_REQUESTS §2): every edge with an
      // endpoint anywhere in the subtree, each carrying `class` + containers. One
      // call replaces the deep node refetch; the engine pre-classifies, so the client
      // drops `internal` edges and resolves owners straight off the edges.
      const hasPortFolders = children.some(
        (c) => exposedPorts(facetFor(c.uid, rawFacet(c.properties))).length > 0,
      );
      if (hasPortFolders) {
        const sub = await getSubtreeEdges(currentParentUid);
        if (gen < lastAppliedReloadGen.current) return; // a newer reload won the race
        scopedEdges = sub;
      }
      // Stash ALL scoped edges (incl. cross-folder ones, which don't live in the
      // store) so grouping can detect boundaries through folder members' exposed
      // ports (chained exposure). The store only keeps inEdges.
      scopedEdgesRef.current = scopedEdges;
      // Reverse index of exposed ports in THIS view: a child-prop uid → the visible
      // folder projecting it as a port (+ the prop-subscription set; lib/routing).
      // Built BEFORE partition because routing treats an edge touching one of these
      // ports as in-view even when both component-uids are off-canvas (a
      // folder-of-folders view). (FACET_DESIGN.md §9.)
      const { index: exposedIndex, subProps } = exposedPortIndex(children);
      // Drop edges the engine classified as internal to one child folder (e.g. a
      // loopback from a folder's exposed output back to a deep input inside the SAME
      // folder) — they belong to the inner view, not this one. (API_REQUESTS §2.)
      const renderEdges = scopedEdges.filter((e) => e.class !== "internal");
      // Partition edges (lib/routing, tested): inEdges (both ends direct-child nodes)
      // draw via the store; crossEdges (touch a folder port and/or one off-canvas
      // end) become a port-to-port edge or a ghost below; both-off edges are dropped.
      const { inEdges, crossEdges } = partitionEdges(renderEdges, childUids, exposedIndex);
      useStructural.getState().setNodes(children, inEdges);
      // Linked-prop flags for the table: every endpoint of every scoped edge,
      // including cross-folder ones (which don't live in the store's `edges`).
      const linked = new Set<number>();
      for (const e of scopedEdges) {
        if (e.sourcePropertyUid != null) linked.add(e.sourcePropertyUid);
        if (e.targetPropertyUid != null) linked.add(e.targetPropertyUid);
      }
      useStructural.getState().setLinkedProps(linked);

      // Build ghost nodes + their cross-folder edges. Cross-folder edges that
      // share the same visible-side (component, property) are GROUPED into one
      // ghost so an output that fans out to N external inputs doesn't render
      // N overlapping ghost boxes at the same Y. The ghost shows the first
      // target inline and surfaces the rest in a click-to-expand popover.
      interface GhostGroup {
        visibleUid: number;
        visiblePropUid: number;
        rowIdx: number;
        side: "input" | "output";
        connections: import("./components/FunctionBlock").GhostConnection[];
        edgeUids: number[];
        visibleX: number;
        visibleY: number;
      }
      // Subscribe the exposed (off-canvas) child props AND their child's __facets
      // prop at PROPERTY level so both the value and the presentation metadata
      // stream live. (Component-level subs only cover visible nodes.) Unioned with
      // any drawer-widget prop subs (e.g. an open SchedulePanel) via flushPropSubs.
      exposedPropSubsRef.current = subProps;
      flushPropSubs();
      const portEdges: RfEdge[] = [];

      const ghostGroups = new Map<string, GhostGroup>();
      for (const e of crossEdges) {
        const route = classifyCrossEdge(e, childUids, exposedIndex);
        const style =
          route.loopBack
            ? { stroke: "hsl(var(--muted-foreground))", strokeWidth: 1.5, strokeDasharray: "6 4" }
            : { stroke: "hsl(var(--cool))", strokeWidth: 1.5 };
        // Both ends visible (node↔port, port↔port, node↔node-via-port): a normal
        // edge straight between the two handles (a folder port draws on the folder
        // node with the deep prop uid as its handle id, same as a plain prop).
        if (route.kind === "edge") {
          portEdges.push({
            id: String(route.edgeUid),
            type: EDGE_TYPE,
            source: String(route.source.uid),
            sourceHandle: String(route.source.handle),
            target: String(route.target.uid),
            targetHandle: String(route.target.handle),
            style,
            animated: false,
          });
          continue;
        }
        // Ghost end: resolve the visible side's row. For a folder PORT the row is
        // its index among the folder's exposed ports; for a plain node it's the
        // user-facing prop row. Merge into the per-(component,handle) ghost group.
        const visibleComp = childByUid.get(route.visibleUid);
        if (!visibleComp) continue;
        let visiblePropUid: number;
        if (route.visibleIsPort) {
          visiblePropUid = route.visibleHandle;
        } else {
          const visibleProp = visibleComp.properties[route.visiblePropName];
          if (!visibleProp) continue;
          visiblePropUid = visibleProp.uid;
        }
        // Row Y of the connected prop — user row OR exposed port — interleaved
        // exactly as FunctionBlock lays them out, so the ghost sits inline with it.
        const rowIdx = rowIndexOf(
          visibleComp,
          facetFor(visibleComp.uid, rawFacet(visibleComp.properties)),
          visiblePropUid,
          linked,
        );
        if (rowIdx < 0) continue;
        const key = `${route.visibleUid}:${visiblePropUid}`;
        let group = ghostGroups.get(key);
        if (!group) {
          group = {
            visibleUid: route.visibleUid,
            visiblePropUid,
            rowIdx,
            side: route.side,
            connections: [],
            edgeUids: [],
            visibleX: visibleComp.metadata?.position?.x ?? 0,
            visibleY: visibleComp.metadata?.position?.y ?? 0,
          };
          ghostGroups.set(key, group);
        }
        group.connections.push({
          externalComponentUid: route.externalUid,
          externalPath: route.externalPath,
          externalPropName: route.externalPropName,
          edgeUid: route.edgeUid,
        });
        group.edgeUids.push(route.edgeUid);
      }

      const ghostNodes: RfNode<GhostNodeData>[] = [];
      const ghostEdges: RfEdge[] = [];
      for (const g of ghostGroups.values()) {
        // Width tailored to the FIRST connection's collapsed label: just the
        // component's own name + prop name (the pill hugs the row; the full
        // path lives in the hover title and the popover).
        const first = g.connections[0];
        const labelPath = lastSegment(first.externalPath);
        const gw = ghostWidthFor(labelPath, first.externalPropName) + (g.connections.length > 1 ? 26 : 0);
        const gx = g.side === "input" ? g.visibleX + NODE_W + GHOST_GAP : g.visibleX - gw - GHOST_GAP;
        const gy = g.visibleY + FB_TITLE_H + g.rowIdx * FB_ROW_H + (FB_ROW_H - GHOST_H) / 2;
        const ghostId = `ghost:${g.visibleUid}:${g.visiblePropUid}`;
        const handleId = `gh:${g.visibleUid}:${g.visiblePropUid}`;
        ghostNodes.push({
          id: ghostId,
          type: "ghost",
          position: { x: gx, y: gy },
          width: gw,
          // selectable: false would strip pointer events on the wrapper in
          // some RF configs, defeating the popover. Keep selectable + harmless;
          // the doc-level click handler skips ghost ids so it doesn't latch
          // selection visually. Still non-draggable.
          draggable: false,
          data: {
            connections: g.connections,
            handleId,
            side: g.side,
            anchorUid: g.visibleUid,
            anchorRowIdx: g.rowIdx,
            width: gw,
            onNavigate: goToComponent,
            onDeleteEdge: deleteGhostEdge,
          },
        });
        // All edges in this group share the same ghost handle id. The visible
        // end uses the real prop uid as its handle id (FunctionBlock renders
        // its Handles with id={String(p.uid)}).
        const visibleHandleId = String(g.visiblePropUid);
        for (const edgeUid of g.edgeUids) {
          // Reconstruct edge from the same crossEdges entry so we have the
          // loopBack / stroke choice. Slightly redundant but keeps the
          // grouping logic linear.
          const e = crossEdges.find((x) => x.uid === edgeUid)!;
          const externalIsTarget = g.side === "input";
          ghostEdges.push({
            id: String(edgeUid),
            type: EDGE_TYPE,
            source: externalIsTarget ? String(g.visibleUid) : ghostId,
            sourceHandle: externalIsTarget ? visibleHandleId : handleId,
            target: externalIsTarget ? ghostId : String(g.visibleUid),
            targetHandle: externalIsTarget ? handleId : visibleHandleId,
            style:
              e.loopBack === true
                ? { stroke: "hsl(var(--muted-foreground))", strokeWidth: 1.5, strokeDasharray: "6 4" }
                : { stroke: "hsl(var(--cool))", strokeWidth: 1.5 },
            animated: false,
          });
        }
      }
      // Capture current selection by passing the existing nodes array to the builder.
      // Using the functional form of setNodes guarantees we see the latest state at
      // the moment React applies it — critical when a click and a topology reload
      // batch together.
      setNodes((prev) => {
        const selectedIds = new Set<string>();
        for (const n of prev) if (n.selected) selectedIds.add(n.id);
        const real = buildRfNodes(
          children,
          enter,
          openNodeContextMenu,
          selectedIds,
          actionTypesRef.current,
        );
        return [...real, ...ghostNodes];
      });
      // Stash edges; the useNodesInitialized effect below will move them into the live
      // `edges` state once React Flow has registered handle positions for every node.
      setEdges([]);
      setPendingEdges([...buildRfEdges(inEdges, children), ...ghostEdges, ...portEdges]);
      // NOTE: subscription is no longer set here. VisibilitySub owns it — it
      // subscribes only the components in/near the viewport (debounced), which
      // fires shortly after these nodes mount. Subscribing all folder children
      // here would stream the off-screen majority for nothing.
    } catch (e) {
      reportError(e);
    }
  }, [currentParentUid, enter, openNodeContextMenu, goToComponent, deleteGhostEdge]);

  // The WS effect captures its handlers once (`[]` deps), so anything it calls
  // must reach the LATEST reload — otherwise a topology event (e.g. after adding
  // a component) fires a stale reload bound to the root level and snaps the view
  // back to root. Keep a ref to the current reload for those call sites.
  const reloadRef = useRef(reload);
  reloadRef.current = reload;

  // Paste = server-side clone of the copied components via POST /copy/nodes
  // (internal edges auto-included), then a single bulkUpdate to translate the
  // clones so their group centroid lands at the cursor (preserving relative
  // layout). The engine auto-suffixes names and assigns uids — no client-side
  // spec building, name collision handling, or edge reconstruction.
  const pasteFromClipboard = useCallback(async () => {
    const cb = clipboardRef.current;
    if (!cb || cb.uids.length === 0) return;
    try {
      // One gesture id across the copy + reposition so paste is a single atomic
      // undo entry (the `copy` plus the clones' `updateMetadata`/facet writes).
      const newUids = await withGesture(async () => {
        const res = await copyNodes({
          componentUids: cb.uids,
          destParentUid: currentParentUid,
          includeInternalEdges: true,
        });
        const clones = res.nodes ?? [];
        if (clones.length === 0) {
          setError({ message: "paste: nothing cloned (sources may have been deleted)" });
          return null;
        }
        // Plan the paste: flatten the cloned subtree, translate the TOP-LEVEL
        // clones so their bounding-box centre lands at the cursor, and remap uid
        // references in any copied __facets (lib/paste, tested). The single
        // bulkUpdate is NON-FATAL — a rejected facet must not abort the paste
        // (else clones are left unselected + the view un-reloaded).
        const cursor = rf.screenToFlowPosition(mouseScreenPos.current);
        const { updates, newUids } = planPaste(clones, currentParentUid, cursor, {
          uidMap: res.uidMap,
          copyCursor: cb.cursor, // preserve the copy-time grab offset
        });
        try {
          if (updates.length > 0) await bulkUpdate(updates);
        } catch (e) {
          console.error("paste: reposition/facet-remap failed:", (e as Error).message);
        }
        return newUids;
      });
      if (newUids == null) return;
      setPendingPasteSelection(newUids);
      // Undo is engine-side now: the gesture was journaled as one entry, so
      // Cmd/Z reverts the whole paste without the client tracking an inverse.
      await reload();
    } catch (e) {
      reportError(e);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [currentParentUid, reload, rf]);

  // Undo / redo are first-class on the engine (it journals each mutation and
  // inverts it). We just call /undo and /redo, scoped to this client's actor
  // stack via the X-Actor-Id header, and reload to reflect the result. The
  // stack is per-actor (spans folders), so an undo may touch another folder;
  // the engine emits topology pushes either way, and reload() refreshes the
  // current view. `ok:false` (e.g. nothing to undo, or a stale precondition)
  // surfaces its reason to the banner.
  const undo = useCallback(async () => {
    try {
      const r = await undoChange();
      if (r.ok) await reload();
      else if (r.reason) reportError(new Error(`Can't undo: ${r.reason}`));
    } catch (e) {
      reportError(e);
    }
  }, [reload]);

  const redo = useCallback(async () => {
    try {
      const r = await redoChange();
      if (r.ok) await reload();
      else if (r.reason) reportError(new Error(`Can't redo: ${r.reason}`));
    } catch (e) {
      reportError(e);
    }
  }, [reload]);

  // Cmd/Ctrl + C / V — window-level so the canvas doesn't need explicit
  // focus. Skipped when focus is in a text editing context so paste-into-
  // input still works normally for the override prompt / palette filter etc.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const cmd = e.metaKey || e.ctrlKey;
      // Cmd/Ctrl+F → component finder. Handled BEFORE the input-focus guard so
      // it overrides the browser's find-in-page even while a field is focused
      // (the finder is more useful here than ctrl-F over the DOM text).
      if (cmd && e.key.toLowerCase() === "f") {
        e.preventDefault();
        setFindOpen(true);
        return;
      }
      // Everything below is skipped while editing text so native copy/paste/
      // undo work in inputs (palette filter, override prompt, value editor).
      const ae = document.activeElement;
      if (
        ae &&
        (ae.tagName === "INPUT" ||
          ae.tagName === "TEXTAREA" ||
          (ae as HTMLElement).isContentEditable)
      ) {
        return;
      }
      if (!cmd) return;
      const key = e.key.toLowerCase();
      if (key === "c") {
        // If the user has TEXT selected (e.g. in the debug log or a read-only
        // example), let the browser copy it natively — only copy the canvas
        // component selection when there's no text selection.
        const sel = window.getSelection();
        if (sel && !sel.isCollapsed && sel.toString().trim()) return;
        e.preventDefault();
        copySelectionToClipboard();
      } else if (key === "v") {
        e.preventDefault();
        void pasteFromClipboard();
      } else if (key === "z" && !e.shiftKey) {
        // Cmd/Ctrl+Z → undo.
        e.preventDefault();
        void undo();
      } else if ((key === "z" && e.shiftKey) || key === "y") {
        // Cmd/Ctrl+Shift+Z (and Ctrl+Y) → redo.
        e.preventDefault();
        void redo();
      } else if (key === "d" && e.shiftKey) {
        // Cmd/Ctrl+Shift+D → toggle the click-debug overlay.
        e.preventDefault();
        setClickDebugOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [copySelectionToClipboard, pasteFromClipboard, undo, redo]);

  useEffect(() => {
    reload();
  }, [reload]);

  // Post-navigation focus: when goToComponent set focusAfterLoad and the new
  // view's nodes have populated to include that target, center the viewport
  // on the target and mark it selected. Keeps current zoom — the goal is "I
  // can see where I jumped to", not "fit everything". Cleared once applied
  // so a subsequent unrelated nodes update doesn't re-trigger.
  useEffect(() => {
    if (focusAfterLoad == null) return;
    const targetId = String(focusAfterLoad);
    const target = nodes.find((n) => n.id === targetId);
    if (!target || target.type === "ghost") return;
    // Estimate the node's center. Width is NODE_W; height is the row-count
    // formula from FunctionBlock so we don't have to wait for RF measurement.
    const restComp = useStructural.getState().components.get(focusAfterLoad);
    const userPropCount = restComp
      ? Object.values(restComp.properties).filter(
          (p) => (p.systemRole ?? ROLE_NORMAL) === ROLE_NORMAL,
        ).length
      : 4;
    const FB_TITLE = 40;
    const FB_ROW = 18;
    const estH = FB_TITLE + userPropCount * FB_ROW + 4;
    const cx = target.position.x + NODE_W / 2;
    const cy = target.position.y + estH / 2;
    rf.setCenter(cx, cy, { duration: 400, zoom: rf.getZoom() });
    setNodes((ns) =>
      ns.map((n) => {
        const want = n.id === targetId;
        return n.selected === want ? n : { ...n, selected: want };
      }),
    );
    setFocusAfterLoad(null);
  }, [nodes, focusAfterLoad, rf]);

  // Detect a folder change. A goToComponent jump sets focusAfterLoad itself, so
  // skip those — this is for plain drill-in / up / breadcrumb navigation.
  useEffect(() => {
    if (prevParentRef.current === currentParentUid) return;
    const from = prevParentRef.current;
    prevParentRef.current = currentParentUid;
    if (focusAfterLoad == null) setNavAfterLoad({ from });
  }, [currentParentUid, focusAfterLoad]);

  // Apply once the new level's nodes have loaded (all belong to currentParentUid).
  useEffect(() => {
    if (!navAfterLoad) return;
    const comps = useStructural.getState().components;
    const real = nodes.filter((n) => n.type === "fb");
    const loaded =
      real.length === 0
        ? comps.size === 0
        : real.every((n) => comps.get(Number(n.id))?.parent === currentParentUid);
    if (!loaded) return; // still showing the old level — wait for the reload
    // Went UP: the folder we left is now a visible node → center + select it.
    if (real.some((n) => n.id === String(navAfterLoad.from))) {
      setFocusAfterLoad(navAfterLoad.from);
    } else if (real.length > 0) {
      // Went DOWN / elsewhere → fit the whole level into view.
      void rf.fitView({ nodes: real.map((n) => ({ id: n.id })), padding: 0.25, duration: 400 });
    }
    setNavAfterLoad(null);
  }, [nodes, navAfterLoad, currentParentUid, rf]);

  // Post-paste selection: once the reload after a paste has populated the
  // new uids into nodes, mark exactly those nodes selected and clear
  // everything else. Waits for ALL pasted uids to appear before firing so
  // a partial reload doesn't half-select a subset.
  useEffect(() => {
    if (pendingPasteSelection == null) return;
    const wantedIds = new Set(pendingPasteSelection.map(String));
    let foundCount = 0;
    for (const n of nodes) if (wantedIds.has(n.id)) foundCount++;
    if (foundCount < wantedIds.size) return;
    setNodes((ns) =>
      ns.map((n) => {
        if (n.type === "ghost") return n;
        const want = wantedIds.has(n.id);
        return n.selected === want ? n : { ...n, selected: want };
      }),
    );
    setEdges((es) => es.map((e) => (e.selected ? { ...e, selected: false } : e)));
    setPendingPasteSelection(null);
  }, [nodes, pendingPasteSelection]);

  // Gate: every endpoint of every pending edge must have its handle registered in
  // React Flow's internal store. Handle registration happens in a useEffect inside the
  // Handle component (after mount + measurement); subscribing to `nodeLookup` here
  // re-evaluates on every internal store change, so we flush exactly when ready.
  // Promote parked edges to the live `edges` array as their handles mount.
  // Checked PER EDGE: an edge whose handles can never resolve (e.g. a malformed
  // output→output edge persisted by the engine — its "target" handle is a source
  // handle) is skipped instead of blocking EVERY edge from rendering. Returns a
  // stable string key (the ready edge ids) so the selector only re-renders when
  // the ready SET changes, not on every store tick.
  const readyKey = useRfStore((s) => {
    if (!pendingEdges) return "";
    const lookup = (s as unknown as { nodeLookup: Map<string, unknown> }).nodeLookup;
    if (!lookup) return "";
    const ids: string[] = [];
    for (const e of pendingEdges) {
      const src = lookup.get(e.source) as
        | { internals?: { handleBounds?: { source?: { id?: string | null }[] | null } } }
        | undefined;
      const dst = lookup.get(e.target) as
        | { internals?: { handleBounds?: { target?: { id?: string | null }[] | null } } }
        | undefined;
      const srcBounds = src?.internals?.handleBounds?.source;
      const dstBounds = dst?.internals?.handleBounds?.target;
      if (!srcBounds || !dstBounds) continue;
      if (!srcBounds.some((h) => h.id === e.sourceHandle)) continue;
      if (!dstBounds.some((h) => h.id === e.targetHandle)) continue;
      ids.push(e.id);
    }
    return ids.join(",");
  });
  useEffect(() => {
    if (!pendingEdges) return;
    const ready = new Set(readyKey ? readyKey.split(",") : []);
    setEdges(pendingEdges.filter((e) => ready.has(e.id)));
    if (ready.size === pendingEdges.length) setPendingEdges(null);
  }, [readyKey, pendingEdges]);
  useEffect(() => {
    // Grace period: the ready edges are already live; stop tracking so any
    // still-unresolved (malformed) edges are dropped rather than pinning
    // pendingEdges and re-running the selector on every store change.
    if (!pendingEdges) return;
    const t = window.setTimeout(() => setPendingEdges(null), 1500);
    return () => window.clearTimeout(t);
  }, [pendingEdges]);

  // Available component types grouped by extension (the palette). GET /schema
  // (via the transport seam's request(), not a second raw-fetch path — see
  // slice-1-wiresheet-transport-seam.md) returns each extension's component
  // definitions; the full type string is `<vendor>-<ext>::<name>`. Components
  // are deduped across extension instances.
  useEffect(() => {
    getSchema()
      .then((data) => {
        const exts = data as unknown as Array<{
          vendor: string;
          name: string;
          version?: string;
          components?: Array<{ name: string; icon?: string; actions?: ActionDef[]; role?: string; singleton?: boolean }>;
        }>;
        // Index schema-declared enum choices (e.g. severity low:0,medium:1,high:2)
        // so those props render as labels + dropdowns like facet aliases.
        loadSchemaChoices(data as unknown as Parameters<typeof loadSchemaChoices>[0]);
        const seen = new Map<string, PaletteExtension>();
        // Same pass builds the action index: type → its action signatures.
        const actions = new Map<string, ActionDef[]>();
        for (const e of exts) {
          const id = `${e.vendor}-${e.name}`;
          let group = seen.get(id);
          if (!group) {
            group = { id, vendor: e.vendor, name: e.name, version: e.version, components: [] };
            seen.set(id, group);
          }
          const have = new Set(group.components.map((c) => c.type));
          for (const c of e.components ?? []) {
            const type = `${id}::${c.name}`;
            if (c.actions && c.actions.length > 0 && !actions.has(type)) {
              actions.set(type, c.actions);
            }
            if (have.has(type)) continue;
            have.add(type);
            // Services (role "*.service") and other singletons (root, ServicesFolder)
            // are engine-managed / auto-generated — the user can't place them, so
            // keep them out of the palette. Their actions are still indexed above
            // for the UI extensions that call them (e.g. the JS store's getApi).
            if (c.singleton === true || (typeof c.role === "string" && c.role.endsWith(".service"))) continue;
            group.components.push({ name: c.name, type, icon: c.icon });
          }
        }
        setActionsByType(actions);
        actionTypesRef.current = new Set(actions.keys());
        // Drop extensions that don't expose any creatable components — they'd render
        // as a dead-end disclosure with nothing inside.
        const list = [...seen.values()].filter((g) => g.components.length > 0);
        list.sort((a, b) => a.id.localeCompare(b.id));
        for (const g of list) g.components.sort((a, b) => a.name.localeCompare(b.name));
        setPalette(list);
      })
      .catch(() => {});
  }, []);

  // Stamp `hasActions` onto already-built nodes when the action index arrives
  // after the first render (build-time stamping in buildRfNodes covers later
  // reloads; this covers the initial schema-load race).
  useEffect(() => {
    const comps = useStructural.getState().components;
    setNodes((ns) => {
      let changed = false;
      const next: AnyNode[] = ns.map((n) => {
        if (n.type !== "fb") return n;
        const fb = n as RfNode<FunctionBlockData>;
        const t = comps.get(Number(fb.id))?.type;
        const has = t ? actionsByType.has(t) : false;
        if (fb.data.hasActions === has) return n;
        changed = true;
        return { ...fb, data: { ...fb.data, hasActions: has } };
      });
      return changed ? next : ns;
    });
  }, [actionsByType]);

  // Diagnostics — long-task observer, frame-time percentiles, and the reporter
  // that streams snapshots to the dev sink (POST /__diag). Started once.
  useEffect(() => {
    startDiagnostics();
    startDiagReporter(1000);
    return () => {
      stopDiagReporter();
      stopDiagnostics();
    };
  }, []);

  // Value subscription = the union of what the graph viewport needs and what the
  // table view shows (so off-screen table rows still stream). Each source writes
  // its set into a ref and triggers a flush.
  const graphSubsRef = useRef<Set<number>>(new Set());
  const tableSubsRef = useRef<Set<number>>(new Set());
  const flushSubs = useCallback(() => {
    const union = new Set<number>([...graphSubsRef.current, ...tableSubsRef.current]);
    streamRef.current?.setSubscriptions(union);
    diagGauges.subscribedComponents = union.size;
  }, []);
  // Stable callback for VisibilitySub — inline would churn its effect on every
  // App re-render (frequent during a pan as nodes mount/unmount).
  const onVisibleSubscription = useCallback(
    (uids: Set<number>) => {
      graphSubsRef.current = uids;
      flushSubs();
    },
    [flushSubs],
  );
  const onTableRows = useCallback(
    (uids: number[]) => {
      tableSubsRef.current = new Set(uids);
      flushSubs();
    },
    [flushSubs],
  );

  // Property-level subscription = the union of the exposed-port set (off-canvas
  // child props rendered as folder ports) and what a drawer widget asks for (e.g.
  // an open SchedulePanel subscribing its config/out/active/nextChange so it
  // streams live regardless of folder). Mirrors the component-sub union above.
  const exposedPropSubsRef = useRef<Set<number>>(new Set());
  const widgetPropSubsRef = useRef<Set<number>>(new Set());
  const flushPropSubs = useCallback(() => {
    const union = new Set<number>([...exposedPropSubsRef.current, ...widgetPropSubsRef.current]);
    streamRef.current?.setPropSubscriptions(union);
  }, []);
  // Threaded to widgets via RenderCtx.subscribeProps: register a desired prop set,
  // get a cleanup that drops it. Replaces the whole widget set each call (one
  // drawer widget is active at a time), so closing/switching panels frees subs.
  const subscribeProps = useCallback(
    (propUids: number[]) => {
      widgetPropSubsRef.current = new Set(propUids);
      flushPropSubs();
      return () => {
        widgetPropSubsRef.current = new Set();
        flushPropSubs();
      };
    },
    [flushPropSubs],
  );

  // Stable adapter over the module-level streamRef singleton so the DiagPanel's
  // rate controls always hit the live stream (streamRef is null at first render,
  // set later in an effect — a stable adapter reads it lazily each call).
  // Throttle setRate (leading + trailing, 200ms) so nothing can flood the
  // engine with rate changes — historically a setRate stream from the zoom
  // controller crashed it. First call goes out immediately; further calls in
  // the window coalesce and the last one flushes at the end.
  const rateThrottle = useRef<{ timer: number | null; pending: number | null }>({
    timer: null,
    pending: null,
  });
  const wsAdapter = useMemo(
    () => ({
      setRate: (hz: number) => {
        const t = rateThrottle.current;
        if (t.timer != null) {
          t.pending = hz; // in the window — remember the latest, flush on timeout
          return;
        }
        streamRef.current?.setTickHz(hz);
        t.pending = null;
        t.timer = window.setTimeout(() => {
          t.timer = null;
          if (t.pending != null) {
            streamRef.current?.setTickHz(t.pending);
            t.pending = null;
          }
        }, 200);
      },
      getRate: () => streamRef.current?.getTickHz() ?? null,
    }),
    [],
  );

  // Zoom-adaptive push rate. When on, the ZoomRateController scales the WS
  // tick rate to the zoom level (low when zoomed out — you can't read values
  // anyway — full when zoomed in). `rateCeiling` is the upper bound the auto
  // mode scales WITHIN; it's the rate the manual buttons set. Kept SEPARATE
  // from the live wsClient rate (which auto-mode drives down/up) so auto-mode
  // can't ratchet its own ceiling. Both persisted.
  // Default OFF AGAIN. The setRate fix (89b01e0) is INCOMPLETE — re-enabling
  // auto-rate (which streams setRate on zoom) crashed the engine again
  // (API_GAPS #12, reopened): REST went to HTTP 000, confirmed engine-side by
  // an external WS probe that couldn't even get a schema. Until setRate is
  // provably crash-proof under sustained/continuous use, we do NOT auto-send
  // it. Opt-in only.
  const [autoRate, setAutoRate] = useState<boolean>(() => {
    try {
      // Default ON (auto-scale push rate with zoom); only an explicit stored "0"
      // keeps it off.
      const v = window.localStorage.getItem("ce-ui.autoRate");
      return v === null ? true : v === "1";
    } catch {
      return true;
    }
  });
  // Manual rate (used only when auto-rate is OFF). Persisted. When auto is on,
  // the ZoomRateController owns the rate (zoom → bucket) and the manual value
  // is ignored — no "ceiling" coupling, which is what left it stuck at 5.
  const [manualRate, setManualRate] = useState<number>(() => {
    try {
      const v = Number(window.localStorage.getItem("ce-ui.manualRate"));
      return Number.isFinite(v) && v >= 1 ? v : 10;
    } catch {
      return 10;
    }
  });
  useEffect(() => {
    try {
      window.localStorage.setItem("ce-ui.autoRate", autoRate ? "1" : "0");
      window.localStorage.setItem("ce-ui.manualRate", String(manualRate));
    } catch {
      /* ignore */
    }
  }, [autoRate, manualRate]);
  // Manual rate pick (auto OFF): apply it live immediately.
  const onSetManualRate = useCallback((hz: number) => {
    setManualRate(hz);
    streamRef.current?.setTickHz(hz);
  }, []);
  // When auto-rate is turned OFF, snap the live rate to the manual value so the
  // session doesn't get stuck at whatever zoom-driven value was last sent.
  useEffect(() => {
    if (!autoRate) streamRef.current?.setTickHz(manualRate);
  }, [autoRate, manualRate]);

  // Our display name for presence: an optional user-chosen base (shared across
  // this browser's tabs via localStorage) PLUS a per-tab discriminator so two
  // tabs of the same browser don't show identical names. The discriminator is
  // generated fresh per load (module scope below) — NOT persisted — so it's
  // unique per tab even for a duplicated tab (which copies storage but re-runs
  // module init). Colors already differ per session; this makes names differ
  // too. Click the name in the PresenceBar to set the base.
  const userName = useMemo(() => {
    let base = "user";
    try {
      base = window.localStorage.getItem("ce-ui.userName") || "user";
    } catch {
      /* ignore */
    }
    return `${base}-${TAB_SUFFIX}`;
  }, []);

  // Publish our presence (selection + folder) whenever it changes, debounced
  // so a drag-select sweep doesn't flood the relay. The engine fans this out
  // to other sessions; we never receive our own.
  const selectedUidsKey = nodes
    .filter((n) => n.selected && n.type !== "ghost")
    .map((n) => n.id)
    .join(",");
  // Stable publisher of our current presence — used both by the on-change
  // effect and the heartbeat. Reads the latest selection key via a ref so the
  // heartbeat interval doesn't need to re-subscribe on every selection change.
  const selKeyRef = useRef(selectedUidsKey);
  selKeyRef.current = selectedUidsKey;
  const publishPresence = useCallback(() => {
    // Presence is direct-mode-only in v1 (see slice-1-wiresheet-transport-seam.md
    // "What must move behind the seam" — a bridge transport drops it, a known,
    // named gap, not this slice's to fix). Duck-type the extra method rather
    // than `instanceof DirectStream` so a test's mock direct-like stream still
    // works without importing the concrete class.
    const key = selKeyRef.current;
    const s = streamRef.current as DirectEngineStream | null;
    s?.publishPresence?.({
      userName,
      selectedComponents: key ? key.split(",").map(Number) : [],
      parentUid: currentParentUid,
    } satisfies PresenceState);
  }, [userName, currentParentUid]);
  useEffect(() => {
    const t = window.setTimeout(publishPresence, 150);
    return () => window.clearTimeout(t);
  }, [selectedUidsKey, currentParentUid, userName, publishPresence]);

  // Heartbeat + TTL sweep. The engine evicts dead sessions only on a slow grace
  // timer, so stale collaborators ("9 others" when there are 2) pile up across
  // reconnects. We republish our presence every HEARTBEAT_MS so live peers stay
  // fresh, and sweep out any collaborator not heard from within PRESENCE_TTL_MS.
  // Dead sessions stop heartbeating → age out; live ones keep refreshing.
  useEffect(() => {
    const HEARTBEAT_MS = 20000;
    const SWEEP_MS = 8000;
    const PRESENCE_TTL_MS = 50000; // ~2.5 missed heartbeats
    const hb = window.setInterval(publishPresence, HEARTBEAT_MS);
    const sw = window.setInterval(() => usePresence.getState().sweep(PRESENCE_TTL_MS), SWEEP_MS);
    return () => {
      window.clearInterval(hb);
      window.clearInterval(sw);
    };
  }, [publishPresence]);

  // Feed the diag gauges from current state on every render — cheap, and keeps
  // the snapshot's structural numbers honest. The rate/timing data is captured
  // continuously inside diagnostics.ts; these are just point-in-time sizes.
  const totalComponentCount = useStructural((s) => s.components.size);
  diagGauges.visibleNodes = nodes.filter((n) => n.type !== "ghost").length;
  diagGauges.ghostNodes = nodes.filter((n) => n.type === "ghost").length;
  diagGauges.edges = edges.length;
  diagGauges.totalComponents = totalComponentCount;
  diagGauges.wsConnected = metrics.wsConnected;
  diagGauges.reconnects = metrics.reconnectCount;
  diagGauges.lastSeq = metrics.lastSeq;

  // Stream for live values + schema + topology pushes. Singleton so HMR doesn't reopen.
  useEffect(() => {
    if (streamRef.current) return;
    const handlers: StreamHandlers = {
      onSchema: (msg) => {
        // Slim decode table only: { uid, dataType, statusFlags } per streamable
        // property. Structure comes from REST.
        loadSchemaIndices(msg.properties);
        // Bind the REST client to this session — every mutation now carries
        // `X-CE-Session: <sessionId>` so the engine attributes topology events to us.
        setRestSessionId(msg.sessionId);
        sessionIdRef.current = msg.sessionId;
        // Scope the engine undo/redo stack to this tab: derive a stable low-16-bit
        // actor id from the (sessionStorage-persisted) session id so each editor
        // gets its own stack and undo never reverts another tab's change. 0 is the
        // engine's shared stack, so avoid it.
        setRestActorId((djb2(msg.sessionId) & 0xffff) || 1);
        // Don't subscribe to everything; the route handler pushes the visible subset
        // via setDesiredSubscription (in reload()).
      },
      onFrame: (frame) => {
        // STATUS sections (typeTag 0x40) carry per-property uint32 status bits,
        // not values — route them to the statusFlags store. Everything else is
        // a typed value section.
        for (const s of frame.sections) {
          if (s.typeTag === TYPE_STATUS) {
            useStatusFlags.getState().applyStatus(s.uids, s.values as ArrayLike<number>);
          } else {
            useValues.getState().apply(s.uids, s.values as ArrayLike<unknown> as never);
          }
        }
      },
      onTopology: (msg) => {
        if (msg.type === "topologyAdded") {
          // Skip the (expensive, scales-with-sheet) reload if we already have
          // everything this event adds — i.e. we appended it optimistically
          // (onAddNode adds the node, onConnect adds the edge). Anything we DON'T
          // have locally — another session, paste, the Connect-to picker, or a
          // cross-folder edge needing a ghost — still reloads to backfill.
          const st = useStructural.getState();
          const haveAll =
            msg.components.every((c) => st.components.has(c.uid)) &&
            msg.edges.every((e) => st.edges.has(e.uid));
          if (haveAll) return;
          // Only reload if the change touches the CURRENT folder. The store holds
          // only this folder, so an off-folder addition can't be surfaced by a
          // reload — without this gate, churn elsewhere (e.g. an extension adding
          // components every tick) spins a full-sheet reload + re-render of every
          // visible node on each event, which reads as periodic jank.
          const pid = currentParentUidRef.current;
          const relevant =
            // A real child has parent === the current folder AND isn't the folder
            // itself — excludes the engine re-announcing the root/container
            // (uid 0 @ 0) every tick, which is what was spinning reloads at root.
            msg.components.some((c) => c.parent === pid && c.uid !== pid) ||
            msg.edges.some((e) => {
              const sc = propertyToComponent.get(e.sourceProperty);
              const tc = propertyToComponent.get(e.targetProperty);
              return (sc != null && st.components.has(sc)) || (tc != null && st.components.has(tc));
            });
          if (!relevant) return;
          scheduleTopologyReload();
        } else if (msg.type === "topologyRemoved") {
          // Splice the removed nodes/edges out of the live RF state without a refetch.
          // Avoids the click-vs-rebuild race that drops in-flight clicks.
          const dropC = new Set(msg.componentUids.map(String));
          const dropE = new Set(msg.edgeUids.map(String));
          setNodes((ns) => ns.filter((n) => !dropC.has(n.id)));
          setEdges((es) => es.filter((e) => !dropE.has(e.id)));
          for (const uid of msg.componentUids) {
            useStructural.getState().removeComponent(uid);
          }
          for (const uid of msg.edgeUids) {
            useStructural.getState().removeEdge(uid);
          }
        } else if (msg.type === "topologyChanged") {
          // If the property SET changed (added or removed) we have to refetch —
          // REST is the source of truth for the structural shape. Otherwise we
          // patch position / name in place to avoid the click-vs-rebuild race.
          const shapeChanged = msg.components.some(
            (c) => (c.addedProperties && c.addedProperties.length > 0) ||
                   (c.removedProperties && c.removedProperties.length > 0) ||
                   c.parent !== undefined,
          );
          if (shapeChanged) {
            scheduleTopologyReload();
            return;
          }
          // Patch in place — DO NOT rebuild the nodes array. Rebuilding would race
          // with the user's clicks: a topology event arriving in the same React batch
          // as a click swallows the click. We only need to update the fields that
          // changed (position, name) and leave everything else (including .selected)
          // alone.
          //
          // Position updates from a DIFFERENT session (another window, the engine
          // itself, etc.) are tweened over POS_ANIM_MS so the node glides instead of
          // jumping. Our own echoes (we just dragged the node and saved it) snap
          // instantly since the local position is already correct.
          const isOwnEcho = msg.originSessionId === sessionIdRef.current;
          const patches = new Map<
            string,
            { position?: { x: number; y: number }; name?: string }
          >();
          for (const p of msg.components) {
            const id = String(p.uid);
            // Our own drag echoes: drop them entirely. The local drag
            // already has the right position via RF's drag state, and even
            // a no-op setNodes outer-array rebuild during an active drag
            // can stutter RF's internal drag handling. Filtering here means
            // setNodes isn't called at all when the whole message is just
            // drag-echo noise for nodes we're currently moving.
            if (isOwnEcho && draggingNodes.current.has(id) && p.position && !p.name) {
              continue;
            }
            patches.set(id, { position: p.position, name: p.name });
          }
          if (patches.size === 0) return;
          setNodes((ns) =>
            ns.map((n) => {
              // Skip ghost sub-nodes — they have no `name` to patch and their
              // layout is derived in reload() (so a real position update on a
              // visible component will rebuild ghost positions on the next
              // reload anyway).
              if (n.type === "ghost") return n;
              const fb = n as RfNode<FunctionBlockData>;
              const p = patches.get(fb.id);
              if (!p) return n;
              const newPos = p.position ?? fb.position;
              const newName = p.name ?? fb.data.name;
              const samePos = newPos === fb.position;
              const sameName = newName === fb.data.name;
              if (samePos && sameName) return n;
              // (Mid-drag own-echoes were filtered out of `patches` above
              // so they never reach this map — setNodes is skipped entirely
              // when the whole message was just drag-echo noise.)
              // Animate the position if it came from another session and we have
              // somewhere to animate from. Leave `position` at its current value;
              // the rAF tick will write interpolated positions until it lands.
              if (!samePos && !isOwnEcho && p.position) {
                animateNodeTo(fb.id, fb.position, p.position);
                return sameName ? n : { ...fb, data: { ...fb.data, name: newName } };
              }
              // Drop any in-flight tween for this node — we're snapping to the
              // authoritative position (own echo, or non-position-only patch).
              posAnims.current.delete(fb.id);
              return {
                ...fb,
                position: samePos ? fb.position : newPos,
                data: sameName ? fb.data : { ...fb.data, name: newName },
              };
            }),
          );
        }
      },
      onStatus: (s) => {
        if (s === "closed") {
          // Connection dropped — clear collaborators so we don't show stale
          // presence while reconnecting. A fresh snapshot arrives on reconnect.
          usePresence.getState().reset();
        }
      },
    };
    // Presence is direct-mode-only in v1 (see slice-1-wiresheet-transport-seam.md):
    // it doesn't ride the generic EngineTransport/EngineStream interface, so it's
    // wired as an extra, optional argument DirectTransport understands and any
    // other transport ignores. Duck-typed rather than `instanceof DirectTransport`
    // so a test transport that also wants presence can opt in the same way.
    // Call `openStream` AS A METHOD (`transport.openStream(...)`), never via a detached local
    // (`const f = transport.openStream; f(...)`) — a class-based transport whose `openStream`
    // reads `this` (e.g. the control-engine BridgeTransport → `this.bridge`) would see `this`
    // become undefined and throw. Cast the transport, not the extracted function, so the
    // receiver is preserved. Presence is the optional 2nd arg direct-mode understands and any
    // other transport ignores (duck-typed — see the note above).
    const withPresence = transport as EngineTransport & {
      openStream(
        h: StreamHandlers,
        presence?: {
          onPresence(m: { type: "presence"; sessionId: string; state: unknown }): void;
          onPresenceSnapshot(m: {
            type: "presenceSnapshot";
            presences: Array<{ sessionId: string; state: unknown }>;
          }): void;
          onPresenceLeft(m: { type: "presenceLeft"; sessionId: string }): void;
        },
      ): EngineStream;
    };
    const stream = withPresence.openStream(handlers, {
      onPresence: (m) => {
        usePresence.getState().upsert(m.sessionId, (m.state ?? {}) as PresenceState);
      },
      onPresenceSnapshot: (m) => {
        usePresence
          .getState()
          .replaceAll(
            (m.presences ?? []).map((p) => ({
              sessionId: p.sessionId,
              state: (p.state ?? {}) as PresenceState,
            })),
          );
      },
      onPresenceLeft: (m) => {
        usePresence.getState().remove(m.sessionId);
      },
    });
    streamRef.current = stream;
    // Close + clear the stream on unmount (or transport swap). Without this the module-level
    // `streamRef` stays populated after the editor unmounts, so the `if (streamRef.current) return`
    // guard above makes a REMOUNT skip arming — the canvas comes back static (no live values), and
    // the old bus subscription leaks. Nulling it means a fresh mount re-arms against the new transport.
    return () => {
      stream.close();
      if (streamRef.current === stream) streamRef.current = null;
    };
  }, [transport]);

  // Coalesce topology pushes into one reload per microtask. Different from the engine's
  // per-tick coalescing (which is already at most three messages per tick); this guards
  // against any unmodelled bursts.
  const topoTimer = useRef<number | null>(null);
  const scheduleTopologyReload = useCallback(() => {
    if (topoTimer.current != null) return;
    topoTimer.current = window.setTimeout(() => {
      topoTimer.current = null;
      // Always the latest reload — see `reloadRef` note. Without this, a
      // post-add topology event reloads the root instead of the current folder.
      reloadRef.current();
    }, 0);
  }, []);

  // Latest mouse screen position — paste uses this so Cmd+V drops the
  // clipboard at the cursor instead of offsetting from where the original
  // sat. Updated continuously, no React state churn.
  const mouseScreenPos = useRef<{ x: number; y: number }>({
    x: window.innerWidth / 2,
    y: window.innerHeight / 2,
  });
  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      mouseScreenPos.current = { x: e.clientX, y: e.clientY };
    };
    window.addEventListener("mousemove", onMove);
    return () => window.removeEventListener("mousemove", onMove);
  }, []);

  // In-memory clipboard for copy/paste — just the source component UIDs and
  // their on-screen centroid. Paste clones by uid via POST /copy/nodes, so we
  // don't snapshot component/edge data. Single-tab scope. The sources must
  // still exist at paste time (copy clones live components); paste reports an
  // error if they're gone.
  interface ClipboardData {
    uids: number[];
    centroid: { x: number; y: number };
    // Mouse position (flow coords) at copy time, so paste preserves each node's
    // offset from where you grabbed it rather than re-centring on the cursor.
    cursor: { x: number; y: number };
  }
  const clipboardRef = useRef<ClipboardData | null>(null);

  // Mirror the current parent uid into a ref so folder-scoped callbacks stay
  // stable across navigation (no callback identity churn into node data props
  // on every breadcrumb change). Undo/redo are now engine-side (per actor), so
  // there are no client-side undo stacks to key by folder anymore.
  const currentParentUidRef = useRef(currentParentUid);
  useEffect(() => {
    currentParentUidRef.current = currentParentUid;
  }, [currentParentUid]);

  // Set of node ids currently mid-drag. The position-animation rAF and the
  // topology-echo handler check this to skip applying our own position
  // echoes back onto a node the user is actively moving — otherwise the
  // echo (which lags the cursor by a network round-trip) would briefly
  // snap the node backwards on every PATCH response.
  const draggingNodes = useRef(new Set<string>());

  // Throttled position PATCH during drag. The user expects other sessions to
  // see the component moving in near-real-time, not just on drop. 100ms →
  // ~10 Hz updates, smoothed by the receiver's exponential ease.
  //
  // Single throttle window across the whole drag group: when 8 components
  // move together, we want ONE PATCH /bulknodes per tick carrying all 8
  // positions, not 8 separate /nodes/uid/{uid} PATCHes per tick. Engine
  // load drops by ~Nx for multi-select moves, and own-echo broadcasts
  // arrive as a single topology event for the whole group.
  const DRAG_PATCH_MS = 100;
  const dragPatchState = useRef<{
    lastSent: number;
    pending: Map<number, { x: number; y: number }>;
    timer: number | null;
  }>({ lastSent: 0, pending: new Map(), timer: null });

  const flushDragPatch = useCallback(() => {
    const s = dragPatchState.current;
    s.timer = null;
    if (s.pending.size === 0) return;
    s.lastSent = performance.now();
    const updates = [...s.pending.entries()].map(([uid, p]) => ({
      uid,
      position: { x: Math.round(p.x), y: Math.round(p.y) },
    }));
    s.pending.clear();
    // Single-uid: single PATCH (cheaper round-trip than wrapping in
    // bulknodes). Multi-uid: one bulk call.
    if (updates.length === 1) {
      const u = updates[0];
      updateNode(u.uid, { position: u.position }).catch(() => {});
    } else {
      bulkUpdate(updates).catch(() => {
        /* mid-drag bulk errors are silent — drag-stop will surface
           persistent failures via its own PATCHes. */
      });
    }
  }, []);

  const sendDragPatch = useCallback(
    (uid: number, pos: { x: number; y: number }) => {
      const s = dragPatchState.current;
      // Coalesce: the LATEST position for any uid in this throttle window
      // wins. Map.set replaces, so only one entry per uid lands in the next
      // flush regardless of how many onNodeDrag callbacks fired in between.
      s.pending.set(uid, pos);
      const now = performance.now();
      if (now - s.lastSent >= DRAG_PATCH_MS) {
        flushDragPatch();
        return;
      }
      if (s.timer == null) {
        s.timer = window.setTimeout(flushDragPatch, DRAG_PATCH_MS - (now - s.lastSent));
      }
    },
    [flushDragPatch],
  );

  const cancelDragPatch = useCallback((id: string) => {
    const s = dragPatchState.current;
    s.pending.delete(Number(id));
    if (s.timer != null && s.pending.size === 0) {
      window.clearTimeout(s.timer);
      s.timer = null;
    }
  }, []);

  // Forward RF-internal edge changes (selection, removal) into our controlled
  // edge state. We don't need to filter — `selected` toggles from box-selection
  // and explicit selection emits from our document-level handler both come
  // through here, and applyEdgeChanges is idempotent on equal updates.
  const onEdgesChange = useCallback((changes: EdgeChange<RfEdge>[]) => {
    // Drop RF's own `select` changes — the document-level pointer handler owns
    // selection (nodes AND edges), so applying RF's too would fight it. Keep
    // everything else (remove, etc.). Elements stay interactive (clickable).
    setEdges((es) => applyEdgeChanges(changes.filter((c) => c.type !== "select"), es));
  }, []);

  // Right-click on an edge → context menu (Reevaluate, Delete).
  const [edgeMenu, setEdgeMenu] = useState<{ x: number; y: number; edgeId: string } | null>(
    null,
  );
  const onEdgeContextMenu = useCallback((e: React.MouseEvent, edge: RfEdge) => {
    e.preventDefault();
    e.stopPropagation();
    setEdgeMenu({ x: e.clientX, y: e.clientY, edgeId: edge.id });
    // Make sure the right-clicked edge is selected so the menu acts on a
    // visible target.
    setEdges((es) =>
      es.map((ed) => (ed.id === edge.id ? (ed.selected ? ed : { ...ed, selected: true }) : ed)),
    );
  }, []);

  // There's no bulk edge-update endpoint (PATCH /bulknodes is component-only —
  // API_GAPS #16), so multi-edge actions still issue one PATCH /edge/uid/{uid}
  // per edge. But fire them CONCURRENTLY (Promise.all) rather than sequentially
  // awaiting — wall time drops from N×RTT to ~1×RTT.
  const reEvaluateEdges = useCallback(async (ids: number[]) => {
    const results = await Promise.allSettled(
      ids.map((uid) => restUpdateEdge(uid, { reEvaluate: true })),
    );
    const failed = results.find((r) => r.status === "rejected") as PromiseRejectedResult | undefined;
    if (failed) reportError(failed.reason);
  }, []);

  // Promote edges to loopback. Per the OpenAPI: `loopBack` may only be set to
  // `true` — once an edge is marked loopback, the engine never clears it
  // automatically and there's no API to clear it either. The only way out is
  // to delete the edge. Patch the dotted-grey loopback style in place for the
  // edges that succeeded — no full reload just to repaint a few edges.
  const setEdgesLoopBack = useCallback(async (ids: number[]) => {
    if (ids.length === 0) return;
    const results = await Promise.allSettled(
      ids.map((uid) => restUpdateEdge(uid, { loopBack: true })),
    );
    const failed = results.find((r) => r.status === "rejected") as PromiseRejectedResult | undefined;
    if (failed) reportError(failed.reason);
    const ok = ids.filter((_, i) => results[i].status === "fulfilled");
    if (ok.length === 0) return;
    const okSet = new Set(ok.map(String));
    const st = useStructural.getState();
    for (const uid of ok) {
      const e = st.edges.get(uid);
      if (e) st.upsertEdge({ ...e, loopBack: true });
    }
    setEdges((es) =>
      es.map((e) =>
        okSet.has(e.id)
          ? { ...e, style: { stroke: "hsl(var(--muted-foreground))", strokeWidth: 1.5, strokeDasharray: "6 4" } }
          : e,
      ),
    );
  }, []);

  // Drag-to-move → PATCH on drag-stop only. Also drag-along any ghost
  // sub-nodes anchored to the moving component so cross-folder edge stubs
  // stay attached as the parent moves.
  const onNodesChange = useCallback((changes: NodeChange<AnyNode>[]) => {
    setNodes((ns) => {
      // Drop RF's own `select` changes — the document-level pointer handler owns
      // selection. Without this, RF and our handler both toggle and race (that's
      // what made shift-click take several attempts). Everything else (position,
      // dimensions, etc.) still applies.
      const next = applyNodeChanges(changes.filter((c) => c.type !== "select"), ns);
      // Collect new positions of anchor components from this batch so we can
      // recompute the positions of their ghosts in a single pass.
      const movedAnchors = new Map<string, { x: number; y: number }>();
      for (const ch of changes) {
        if (ch.type !== "position" || !ch.position) continue;
        // Only real components are anchors. Ghosts moving themselves (they
        // can't — they're non-draggable) wouldn't be anchors anyway.
        const n = next.find((m) => m.id === ch.id);
        if (!n || n.type === "ghost") continue;
        movedAnchors.set(ch.id, ch.position);
      }
      if (movedAnchors.size === 0) return next;
      return next.map((n) => {
        if (n.type !== "ghost") return n;
        const g = n as RfNode<GhostNodeData>;
        const anchor = movedAnchors.get(String(g.data.anchorUid));
        if (!anchor) return n;
        const gx =
          g.data.side === "input"
            ? anchor.x + NODE_W + GHOST_GAP
            : anchor.x - g.data.width - GHOST_GAP;
        const gy = anchor.y + FB_TITLE_H + g.data.anchorRowIdx * FB_ROW_H;
        return { ...g, position: { x: gx, y: gy } };
      });
    });
    // Debug: capture only select changes — what we're chasing right now. Resolve to
    // component name (instead of UID) so the click-debugger row is readable.
    const selChanges = changes.filter((c) => c.type === "select");
    if (selChanges.length > 0) {
      const comps = useStructural.getState().components;
      const compact = selChanges
        .map((c) => {
          const id = (c as { id: string }).id;
          const sel = (c as { selected: boolean }).selected;
          const name = comps.get(Number(id))?.name ?? id;
          return `${name}=${sel ? "+" : "-"}`;
        })
        .join(" ");
      metrics.lastSelChange = compact;
      metrics.lastSelChangeAt = performance.now();
    }
    for (const ch of changes) {
      if (ch.type === "position" && ch.dragging) {
        // User is interacting with this node — cancel any in-flight tween
        // for it so we don't fight the drag. Drag streaming itself is
        // handled by onNodeDragStart/Drag/Stop below: ch.dragging in
        // NodeChange is unreliable across the first frame of certain RF
        // configurations, so we rely on the explicit drag callbacks
        // instead for the draggingNodes set and the PATCH stream.
        posAnims.current.delete(ch.id);
      }
    }
  }, []);

  // Explicit drag lifecycle from React Flow — used to maintain draggingNodes
  // and stream throttled position PATCHes. Replaces reading the unreliable
  // `dragging` flag off NodeChange entries. Position writes are journaled
  // (`updateMetadata`); a fresh gesture id stamped here groups the whole drag's
  // streamed frames + the final stop write into ONE undo entry.
  const onNodeDragStart = useCallback(
    (_e: unknown, _node: AnyNode, ns: AnyNode[]) => {
      const real = ns.filter((n) => n.type !== "ghost");
      for (const n of real) draggingNodes.current.add(n.id);
      if (real.length > 0) setRestGestureId(newGestureId());
    },
    [],
  );
  const onNodeDrag = useCallback(
    (_e: unknown, _node: AnyNode, ns: AnyNode[]) => {
      for (const n of ns) {
        if (n.type === "ghost") continue;
        sendDragPatch(Number(n.id), n.position);
      }
    },
    [sendDragPatch],
  );
  const onNodeDragStop = useCallback(
    (_e: unknown, _node: AnyNode, ns: AnyNode[]) => {
      // Clear the drag flag(s) BEFORE the final PATCH so the topology echo
      // from this last call applies cleanly (no own-echo suppression).
      const real = ns.filter((n) => n.type !== "ghost");
      for (const n of real) {
        draggingNodes.current.delete(n.id);
        cancelDragPatch(n.id);
      }
      if (real.length === 0) return;
      // Snap to the grid only NOW, on release — the drag itself stays free/smooth.
      const snap = (v: number) => Math.round(v / GRID_GAP) * GRID_GAP;
      const updates = real.map((n) => ({
        uid: Number(n.id),
        position: { x: snap(n.position.x), y: snap(n.position.y) },
      }));
      // Apply the snapped positions locally so the nodes settle onto the grid
      // immediately (otherwise they'd sit at the free-drag spot until a reload).
      const snapped = new Map(updates.map((u) => [String(u.uid), u.position]));
      setNodes((prev) => prev.map((n) => (snapped.has(n.id) ? { ...n, position: snapped.get(n.id)! } : n)));
      // Match the streaming path: single → /nodes/uid/{uid}, multi → /bulknodes.
      // The gesture id (set on drag start) is still active here, so this final
      // write joins the drag's undo group; clear it after dispatching.
      if (updates.length === 1) {
        const u = updates[0];
        updateNode(u.uid, { position: u.position }).catch((e) =>
          reportError(e),
        );
      } else {
        bulkUpdate(updates).catch((e) => reportError(e));
      }
      setRestGestureId(null);
    },
    [cancelDragPatch],
  );

  // Connect — drag from a source handle (output) to a target handle (input). Uses the
  // All node-click selection is handled by the document-level pointer capture
  // listener above. React Flow's own onNodeClick / onPaneClick don't fire reliably
  // when `selectionOnDrag` is enabled, so we bypass them.

  // Connect — drag a source (output) handle to a target (input) handle. Handle IDs
  // are property UIDs.
  const onConnect = useCallback(async (c: Connection) => {
    if (!c.source || !c.target || !c.sourceHandle || !c.targetHandle) return;
    try {
      // Handle ids ARE property uids. A handle on a folder port belongs to a deep
      // child, not the folder it's drawn on — but the engine derives the owning
      // component from the prop uid (`componentUid`, API_REQUESTS §1), so we post
      // just the two prop uids and omit the component uids.
      const created = await restAddEdge({
        sourcePropUid: Number(c.sourceHandle),
        targetPropUid: Number(c.targetHandle),
      });
      if (created?.uid != null) {
        // Fast path: append just this edge instead of reloading the whole sheet.
        // onConnect only fires for a drag between two VISIBLE handles, so both
        // endpoints are on-canvas — the edge is in-folder, no ghost needed. The
        // WS topologyAdded echo is skipped because the store already has it.
        // POST /edge now echoes the full edge (source/target uids + prop uids +
        // loopBack — API_REQUESTS §7), so store it as-is; downstream consumers
        // (grouping boundary detection, exposed-port routing) read those fields.
        useStructural.getState().upsertEdge(created);
        const isLoop = created.loopBack === true;
        const rfEdge: RfEdge = {
          id: String(created.uid),
          type: EDGE_TYPE,
          source: c.source,
          sourceHandle: c.sourceHandle,
          target: c.target,
          targetHandle: c.targetHandle,
          style: isLoop
            ? { stroke: "hsl(var(--muted-foreground))", strokeWidth: 1.5, strokeDasharray: "6 4" }
            : { stroke: "hsl(var(--cool))", strokeWidth: 1.5 },
          animated: false,
        };
        setEdges((es) => (es.some((e) => e.id === rfEdge.id) ? es : [...es, rfEdge]));
      } else {
        await reload(); // unexpected: no edge returned — fall back
      }
    } catch (e) {
      reportError(e);
    }
  }, [reload]);

  // Delete keys: remove selected nodes & edges.
  // Delete real components in one bulk call when there's more than one;
  // ghosts are derived from edges and skipped. Single-component delete
  // still uses /nodes/uid/{uid} since the round-trip is slightly cheaper
  // than wrapping a one-entry batch.
  //
  // Delete is soft-delete, so undo is just `restore` by the deleted uids —
  // no pre-delete snapshot, and it correctly brings back a folder's children
  // (the engine cascades the soft-delete and restore reverses the whole set).
  // React Flow fires onBeforeDelete with the COMPLETE set about to be deleted
  // (it auto-includes edges connected to deleted nodes) before onNodesDelete /
  // onEdgesDelete. We record the node ids here so onEdgesDelete can skip the
  // redundant server delete for edges the engine will cascade-remove with their
  // component. (Deleted nodes' edges don't survive, so a stale set never wrongly
  // skips a still-present edge — no clearing needed.)
  const deletingNodeIds = useRef<Set<string>>(new Set());
  const onBeforeDelete = useCallback(
    async ({ nodes }: { nodes: AnyNode[]; edges: RfEdge[] }) => {
      deletingNodeIds.current = new Set(nodes.filter((n) => n.type !== "ghost").map((n) => n.id));
      return true;
    },
    [],
  );

  // Drop a folder's exposed-port records that point at children which just left
  // it (deleted or reparented out). An exposed port projects a child prop; once
  // that child is gone the port is dangling — its value never streams and it
  // renders as a stale `#uid` row. Records reference the child via `childComponent`
  // (direct) OR the inner folder (chain link), both covered by the uid set. Pure
  // read-modify-write of `folderUid`'s __facets; no-ops when nothing matches.
  const onNodesDelete = useCallback(
    async (ns: AnyNode[]) => {
      const real = ns.filter((n) => n.type !== "ghost");
      if (real.length === 0) return;
      const uids = real.map((n) => Number(n.id));
      try {
        if (uids.length === 1) {
          await restRemoveNode(uids[0]);
        } else {
          await bulkDelete({ componentUids: uids });
        }
        for (const uid of uids) useStructural.getState().removeComponent(uid);
        // The exposure maintainer drops the departed children's ports server-side
        // on remove (EXPOSURE_SPEC §5.1) — no client facet write.
      } catch (e) {
        reportError(e);
      }
      await reload();
    },
    [reload],
  );

  const onEdgesDelete = useCallback(
    async (es: RfEdge[]) => {
      if (es.length === 0) return;
      // Don't send a DELETE for edges whose component is being deleted in the
      // same gesture — the engine cascade-removes them with the component.
      // React Flow still reports them here (even unselected), so we just skip
      // the server call and reconcile local state below.
      const del = deletingNodeIds.current;
      const toSend = es.filter((e) => !del.has(e.source) && !del.has(e.target));
      const uids = toSend.map((e) => Number(e.id));
      try {
        if (uids.length === 1) {
          await restRemoveEdge(uids[0]);
        } else if (uids.length > 1) {
          await bulkDelete({ edgeUids: uids });
        }
      } catch (err) {
        // Safety net for other races (e.g. reconnect-replace): a 404 "edge not
        // found" means it's already gone, which is the intended end state.
        if (!(err instanceof RestError && err.status === 404)) reportError(err);
      }
      // Reconcile local state for ALL reported edges (cascaded ones included).
      for (const e of es) useStructural.getState().removeEdge(Number(e.id));
      setEdges((cur) => cur.filter((e) => !es.find((d) => d.id === e.id)));
    },
    [],
  );

  const onAddNode = useCallback(
    async (type: string, worldPos?: { x: number; y: number }) => {
      const vp = rf.getViewport();
      const pos =
        worldPos ??
        {
          x: Math.round((window.innerWidth / 2 - vp.x) / vp.zoom),
          y: Math.round((window.innerHeight / 2 - vp.y) / vp.zoom),
        };
      // Don't drop a new node exactly on top of an existing one (repeated
      // center-adds would pile up). Cascade diagonally off any node already at
      // this spot. Persisted, so it stays put after reload too.
      {
        const STACK_OFFSET = 16;
        const occupied = (x: number, y: number) =>
          rf.getNodes().some(
            (n) =>
              n.type === "fb" && Math.round(n.position.x) === x && Math.round(n.position.y) === y,
          );
        let guard = 0;
        while (occupied(Math.round(pos.x), Math.round(pos.y)) && guard < 200) {
          pos.x += STACK_OFFSET;
          pos.y += STACK_OFFSET;
          guard += 1;
        }
        pos.x = Math.round(pos.x);
        pos.y = Math.round(pos.y);
      }
      // The engine validates names against a strict charset and the auto-derived default
      // can include `::` from the type → rejected with "Name contains invalid characters".
      // Derive a clean base from the type's local segment and find the first free suffix
      // under the current parent.
      const base = sanitizeName(type);
      const siblings = new Set(
        Array.from(useStructural.getState().components.values())
          .filter((c) => c.parent === currentParentUid)
          .map((c) => c.name),
      );
      const name = uniqueName(base, siblings);
      try {
        const created = await restAddNode({
          type,
          name,
          parentUid: currentParentUid,
          defaultValues: { position: { x: Math.round(pos.x), y: Math.round(pos.y) } },
        });
        if (created?.uid != null) {
          // Add is journaled engine-side (addComponent op), so Cmd/Z removes it.
          // Fast path: append just this node instead of reloading the whole
          // sheet (a full reload re-fetches + rebuilds every node, so on a large
          // sheet it's the add lag spike). restAddNode returns the full
          // component, so no extra fetch is needed; the WS topologyAdded echo
          // for our own session is suppressed (see onTopology).
          useStructural.getState().upsertComponent(created);
          const [rfNode] = buildRfNodes(
            [created],
            enter,
            openNodeContextMenu,
            undefined,
            actionTypesRef.current,
          );
          if (rfNode) {
            setNodes((ns) => (ns.some((n) => n.id === rfNode.id) ? ns : [...ns, rfNode]));
          }
        } else {
          await reload(); // unexpected: no component returned — fall back
        }
      } catch (e) {
        reportError(e);
      }
    },
    [rf, reload, currentParentUid, enter, openNodeContextMenu],
  );

  // Creatable component types for the ConnectPicker's "New" flow.
  const componentTypes = useMemo(
    () =>
      palette.flatMap((g) =>
        g.components.map((c) => ({ name: c.name, type: c.type, group: g.id })),
      ),
    [palette],
  );

  // Add a component by a type hint (e.g. "schedule") — resolve the palette type
  // whose name / last type-segment matches, then create it in the current folder.
  const onAddComponentByHint = useCallback(
    (hint: string) => {
      const h = hint.toLowerCase();
      const seg = (t: string) => t.toLowerCase().split(/[:/.\\]+/).filter(Boolean).pop() ?? t.toLowerCase();
      const match = componentTypes.find((c) => c.name.toLowerCase() === h || seg(c.type) === h);
      if (match) void onAddNode(match.type);
    },
    [componentTypes, onAddNode],
  );

  // Create one component of `type` in the current folder and return it (with its
  // properties) so the caller can wire up to it. Mirrors onAddNode but returns
  // the created Component instead of being fire-and-forget.
  const createComponent = useCallback(
    async (
      type: string,
      opts?: { nearUid?: number; side?: "left" | "right" },
    ): Promise<Component | null> => {
      const baseName = sanitizeName(type);
      const siblings = new Set(
        Array.from(useStructural.getState().components.values())
          .filter((c) => c.parent === currentParentUid)
          .map((c) => c.name),
      );
      const name = uniqueName(baseName, siblings);
      // Place next to the source component when the picker passes one — to its
      // right for an output→input link, to its left for an input←output link —
      // so the new node lands beside it, not at screen center. Fall back to the
      // viewport center when there's no anchor.
      const near =
        opts?.nearUid != null
          ? useStructural.getState().components.get(opts.nearUid)
          : undefined;
      let pos: { x: number; y: number };
      if (near?.metadata?.position) {
        const GAP = 80;
        const dx = (NODE_W + GAP) * (opts?.side === "left" ? -1 : 1);
        pos = { x: (near.metadata.position.x ?? 0) + dx, y: near.metadata.position.y ?? 0 };
      } else {
        const vp = rf.getViewport();
        pos = {
          x: Math.round((window.innerWidth / 2 - vp.x) / vp.zoom),
          y: Math.round((window.innerHeight / 2 - vp.y) / vp.zoom),
        };
      }
      try {
        const created = await restAddNode({
          type,
          name,
          parentUid: currentParentUid,
          defaultValues: { position: { x: Math.round(pos.x), y: Math.round(pos.y) } },
        });
        if (created?.uid != null) {
          // Add is journaled engine-side (addComponent op) → Cmd/Z removes it.
          // Incremental append — same fast path as onAddNode, no full reload.
          useStructural.getState().upsertComponent(created);
          const [rfNode] = buildRfNodes(
            [created],
            enter,
            openNodeContextMenu,
            undefined,
            actionTypesRef.current,
          );
          if (rfNode) setNodes((ns) => (ns.some((n) => n.id === rfNode.id) ? ns : [...ns, rfNode]));
        }
        return created ?? null;
      } catch (e) {
        reportError(e);
        return null;
      }
    },
    [rf, currentParentUid, enter, openNodeContextMenu],
  );

  // Edge add for the Connect-to picker. Appends the edge if both endpoints are
  // in the current view (in-folder); falls back to a reload only when the target
  // is in another folder (needs a ghost). Keeps connect-to-existing AND
  // connect-to-new fast instead of full-reloading.
  const connectEdge = useCallback(
    async (payload: {
      sourceUid: number;
      sourcePropUid: number;
      targetUid: number;
      targetPropUid: number;
    }) => {
      const created = await restAddEdge(payload);
      if (created?.uid == null) return;
      // POST /edge echoes the full edge (uids + prop uids + loopBack — see onConnect
      // / API_REQUESTS §7), so store it as-is.
      useStructural.getState().upsertEdge(created);
      const st = useStructural.getState();
      const inView = st.components.has(payload.sourceUid) && st.components.has(payload.targetUid);
      if (inView) {
        const isLoop = created.loopBack === true;
        const rfEdge: RfEdge = {
          id: String(created.uid),
          type: EDGE_TYPE,
          source: String(payload.sourceUid),
          sourceHandle: String(payload.sourcePropUid),
          target: String(payload.targetUid),
          targetHandle: String(payload.targetPropUid),
          style: isLoop
            ? { stroke: "hsl(var(--muted-foreground))", strokeWidth: 1.5, strokeDasharray: "6 4" }
            : { stroke: "hsl(var(--cool))", strokeWidth: 1.5 },
          animated: false,
        };
        setEdges((es) => (es.some((e) => e.id === rfEdge.id) ? es : [...es, rfEdge]));
      } else {
        await reload(); // cross-folder target → needs a ghost
      }
    },
    [reload],
  );

  // Expose a child's prop as a port on the current container (folder). Writes the
  // container's __facets (read-modify-write of the freshly-fetched value, since
  // the container itself is off-canvas one level up), then reloads.
  // Manually pin a child prop as a port on the current folder (persists with no
  // crossing edge). The engine maintainer derives expose/owner/name; we just pin.
  const exposeProp = useCallback(
    async (childPropUid: number) => {
      try {
        await exposePort(currentParentUid, childPropUid);
        await reload();
      } catch (e) {
        reportError(e);
      }
    },
    [currentParentUid, reload, reportError],
  );

  // Clear a manual pin. The port survives only if a boundary edge still justifies it.
  const unexposeProp = useCallback(
    async (folderUid: number, childPropUid: number) => {
      try {
        await unexposePort(folderUid, childPropUid);
        await reload();
      } catch (e) {
        reportError(e);
      }
    },
    [reload, reportError],
  );

  // Group selected components into a new folder — one server-side op (POST /group):
  // the engine creates the Folder, reparents the selection, and reconciles exposure
  // (the new folder gains its boundary ports; ancestors re-chain). We then nudge the
  // folder to the members' bounding-box centre and reload.
  const groupSelected = useCallback(
    async (uids: number[]) => {
      if (uids.length < 2) return;
      const group = new Set(uids);
      // Folder position = bounding-box CENTER of the members, from the LIVE RF
      // positions (store positions can be stale right after a drag).
      const xs: number[] = [];
      const ys: number[] = [];
      for (const node of rf.getNodes()) {
        if (group.has(Number(node.id))) {
          xs.push(node.position.x);
          ys.push(node.position.y);
        }
      }
      try {
        // One gesture across the group + the position nudge so a single undo
        // reverses the whole thing (the engine coalesces same-gesture writes).
        await withGesture(async () => {
          const folder = await groupComponents({ componentUids: uids, parentUid: currentParentUid });
          if (folder?.uid != null && xs.length) {
            await updateNode(folder.uid, {
              position: {
                x: Math.round((Math.min(...xs) + Math.max(...xs)) / 2),
                y: Math.round((Math.min(...ys) + Math.max(...ys)) / 2),
              },
            });
          }
        });
        await reload();
      } catch (e) {
        reportError(e);
      }
    },
    [currentParentUid, reload, reportError, rf],
  );

  const openDetails = useCallback(async (componentUid: number) => {
    if (!useStructural.getState().components.has(componentUid)) {
      try {
        const resp = await getNodeByUid(componentUid, { depth: 0 });
        const c = resp.nodes[0];
        if (c) useStructural.getState().upsertComponent(c);
      } catch {
        /* fall through — panel shows "no editable properties" */
      }
    }
    setDetailsUid(componentUid);
  }, []);

  const ceCtx = useMemo(
    () => ({
      componentTypes,
      createComponent,
      connectEdge,
      exposeProp,
      unexposeProp,
      openDetails,
      requestReload: scheduleTopologyReload,
      parentName: crumbs.length > 1 ? crumbs[crumbs.length - 1]?.name : undefined,
    }),
    [
      componentTypes,
      createComponent,
      connectEdge,
      exposeProp,
      unexposeProp,
      openDetails,
      scheduleTopologyReload,
      crumbs,
    ],
  );

  // DnD: dragging a palette item into the canvas drops a new component at the cursor.
  const onDragOver = useCallback((e: React.DragEvent) => {
    if (e.dataTransfer.types.includes(DND_TYPE)) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
    }
  }, []);

  const onDrop = useCallback(
    (e: React.DragEvent) => {
      const type = e.dataTransfer.getData(DND_TYPE);
      if (!type) return;
      e.preventDefault();
      const worldPos = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
      onAddNode(type, worldPos);
    },
    [rf, onAddNode],
  );

  // --- Table view (split pane) ---
  const [tableOpen, setTableOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false); // left palette overlay — opened via the left-edge tab (like the right extension drawer)
  const [splitPct, setSplitPct] = useState(55); // graph pane width %
  const splitRestore = useRef(55);
  const tableMaxed = splitPct <= 12;
  // Drawer open/close animation: the graph pane's width transitions, and the
  // drawer stays mounted until the close transition finishes (then unmounts).
  // The transition is suppressed while dragging the split so resizing stays snappy.
  const [drawerMounted, setDrawerMounted] = useState(false);
  const [splitDragging, setSplitDragging] = useState(false);
  useEffect(() => { if (tableOpen) setDrawerMounted(true); }, [tableOpen]);
  const drawerVisible = tableOpen || drawerMounted;
  // Loaded extensions (outer, right-edge tab level) and the active one. Each
  // extension's UIs become the inner side-strip tabs. Stubbed via getExtensions().
  const [extensions, setExtensions] = useState<ExtensionUi[]>([]);
  const [activeExtId, setActiveExtId] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    getExtensions().then((exts) => {
      if (!live) return;
      setExtensions(exts);
      setActiveExtId((cur) => cur ?? exts[0]?.id ?? null);
    });
    return () => {
      live = false;
    };
  }, []);
  const activeExt = extensions.find((e) => e.id === activeExtId) ?? extensions[0] ?? null;
  const openExtension = (id: string) => {
    // Toggle: clicking the already-open extension's tab closes the drawer.
    if (tableOpen && activeExtId === id) { setTableOpen(false); return; }
    setActiveExtId(id);
    setTableOpen(true);
  };

  // Right-click "Open UX": open the drawer to the extension UI that edits this
  // component's type, focused on the component. One-shot `nonce` so re-opening
  // the same uid re-triggers. Availability/behaviour are descriptor-driven
  // (findComponentUx) — stub today, engine GET /ui/list later.
  const uxNonceRef = useRef(0);
  const [uxFocus, setUxFocus] = useState<{ uiId: string; uid: number; nonce: number } | null>(null);
  const openComponentUx = useCallback((uid: number) => {
    const type = useStructural.getState().components.get(uid)?.type;
    const target = findComponentUx(extensions, type);
    if (!target) return;
    setActiveExtId(target.extId);
    setTableOpen(true);
    setUxFocus({ uiId: target.uiId, uid, nonce: ++uxNonceRef.current });
  }, [extensions]);
  // Open the root ext's "Inspect" panel on a component — a full read-only overview.
  // The panel is `follow`, so it binds to the (right-click-)selected component; the
  // focus just opens the drawer and switches to the Inspect tab.
  const openComponentInspect = useCallback((uid: number) => {
    // Select exactly this node so the `follow` Inspect panel binds to it (the focus
    // is a fallback). Also makes it visually clear which component is being inspected.
    setNodes((ns) => ns.map((n) => {
      const want = n.id === String(uid);
      return n.selected === want ? n : { ...n, selected: want };
    }));
    setActiveExtId("ce");
    setTableOpen(true);
    setUxFocus({ uiId: "components-inspect", uid, nonce: ++uxNonceRef.current });
  }, []);
  // Replace the graph selection with exactly the given component uids (the table
  // computes the set, including shift-range / ctrl-toggle).
  const onTableSelect = useCallback((uids: number[]) => {
    const want = new Set(uids.map(String));
    setNodes((ns) =>
      ns.map((n) => {
        const s = want.has(n.id);
        return n.selected === s ? n : { ...n, selected: s };
      }),
    );
  }, []);
  const startSplitDrag = useCallback((e: React.PointerEvent) => {
    e.preventDefault();
    setSplitDragging(true);
    const move = (ev: PointerEvent) => {
      const pct = (ev.clientX / window.innerWidth) * 100;
      setSplitPct(Math.min(90, Math.max(10, pct)));
    };
    const up = () => {
      setSplitDragging(false);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }, []);
  // Spacebar focuses the canvas on the current selection: one node → pan to
  // center it; multiple → zoom to fit them all.
  const focusSelection = useCallback(() => {
    const selNodes = rf.getNodes().filter((n) => n.selected && n.type === "fb");
    if (selNodes.length === 0) return;
    if (selNodes.length === 1) {
      const n = selNodes[0];
      const w = n.width ?? NODE_W;
      const h = n.height ?? 80;
      rf.setCenter(n.position.x + w / 2, n.position.y + h / 2, {
        zoom: rf.getViewport().zoom,
        duration: 350,
      });
    } else {
      void rf.fitView({ nodes: selNodes.map((n) => ({ id: n.id })), padding: 0.25, duration: 350 });
    }
  }, [rf]);
  // Global spacebar → focus the selection (works whether the selection was made
  // in the graph or the table). Ignored while typing in a field.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== " " && e.code !== "Space") return;
      const t = e.target as HTMLElement | null;
      if (
        t &&
        (t.tagName === "INPUT" || t.tagName === "SELECT" || t.tagName === "TEXTAREA" || t.isContentEditable)
      )
        return;
      if (!rf.getNodes().some((n) => n.selected && n.type === "fb")) return;
      e.preventDefault();
      focusSelection();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [focusSelection, rf]);
  // Table inline-edit → override (with duration) / set default / clear.
  const onTableSetOverride = useCallback(
    async (componentUid: number, property: string, value: FlexValue, duration: number) => {
      try {
        await patchOverrides(componentUid, { setOverrides: [{ property, value, duration }] });
      } catch (e) {
        reportError(e);
      }
    },
    [],
  );
  const onTableSetDefault = useCallback(
    async (componentUid: number, property: string, value: FlexValue) => {
      try {
        await updateNode(componentUid, { properties: { [property]: { value } } });
      } catch (e) {
        reportError(e);
      }
    },
    [],
  );
  const onTableClearOverride = useCallback(
    async (componentUid: number, property: string) => {
      try {
        await patchOverrides(componentUid, { clearOverrides: [property] });
      } catch (e) {
        reportError(e);
      }
    },
    [],
  );
  // Selected component uids (from RF node state) for row highlighting.
  const tableSelected = nodes.filter((n) => n.selected).map((n) => Number(n.id));

  return (
    <CeWiresheetContext.Provider value={ceCtx}>
      <style>{EDGE_SELECTED_CSS}</style>
      {/* `.ce-wiresheet` scopes the editor's own design tokens (wiresheet-theme.css)
          so the host palette can't leak in/out; `.theme-light` flips dark↔light. */}
      <div
        className={colorMode === "light" ? "ce-wiresheet theme-light" : "ce-wiresheet"}
        style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column" }}
      >
        {/* Top bar (rbx-styled): folder breadcrumb + palette toggle. */}
        <header className="flex items-center justify-between gap-3 border-b border-border bg-card px-3 py-2 text-foreground">
          <div className="flex items-center gap-2 text-[13px]">
            <Network size={15} className="text-r1" />
            {crumbs.map((c, i) => (
              <span key={c.uid} className="flex items-center gap-2">
                {i > 0 && <span className="text-border">/</span>}
                <button
                  type="button"
                  onClick={() => goToCrumb(i)}
                  className={
                    i === crumbs.length - 1
                      ? "font-semibold text-foreground"
                      : "text-muted-foreground hover:text-foreground"
                  }
                >
                  {c.name}
                </button>
              </span>
            ))}
          </div>
          {/* Connection status + diagnostics handle, lifted from the old bottom
              bar (which is removed to match rbx). */}
          <ConnectionStatus open={diagOpen} onToggle={() => setDiagOpen((o) => !o)} />
        </header>
        {/* Split row: graph | divider | table. The palette is a left-edge OVERLAY
            inside the graph pane (mirrors the right extension strip) — it is not a
            flex column, so opening it never shifts the canvas. */}
        <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        {/* LEFT: graph pane. The transform makes it the containing block for its
            position:fixed overlays (dock, panels, menus) so they stay within the
            pane instead of spilling over the table. */}
        <div
          onTransitionEnd={(e) => {
            if (e.propertyName === "width" && !tableOpen) setDrawerMounted(false);
          }}
          style={{
            position: "relative",
            height: "100%",
            width: tableOpen ? `${splitPct}%` : "100%",
            flexShrink: 0,
            transform: "translateZ(0)",
            overflow: "hidden",
            transition: splitDragging ? "none" : "width 240ms cubic-bezier(0.22, 1, 0.36, 1)",
          }}
        >
      <div
        style={{ position: "absolute", inset: 0 }}
        onDragOver={onDragOver}
        onDrop={onDrop}
        onPointerDown={onCanvasPointerDown}
        onPointerMove={onCanvasPointerMove}
        onPointerUp={onCanvasPointerUp}
        onContextMenu={(e) => {
          // Only suppress the native browser menu. Our pane menu opens on
          // pointer-UP (see onCanvasPointerUp) — the browser fires this on PRESS,
          // before we can tell a click from a drag-select.
          e.preventDefault();
        }}
      >
        <ReactFlow
          colorMode={colorMode}
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onEdgeContextMenu={onEdgeContextMenu}
          onNodeDragStart={onNodeDragStart}
          onNodeDrag={onNodeDrag}
          onNodeDragStop={onNodeDragStop}
          onConnect={onConnect}
          // While dragging a connection, snap to the nearest handle within this
          // radius (px, flow units) — extends the effective hit area a bit past
          // the component edge so you don't have to land exactly on the port,
          // and latches onto the closest one. Default is 20.
          connectionRadius={38}
          onBeforeDelete={onBeforeDelete}
          onNodesDelete={onNodesDelete}
          onEdgesDelete={onEdgesDelete}
          defaultViewport={{ x: 80, y: 80, zoom: 1 }}
          minZoom={0.1}
          maxZoom={2}
          // Cull off-screen nodes/edges from the React render tree. At
          // 100+ components the whole view rendering every node — even
          // those panned offscreen — is the main "feels frozen" cause.
          onlyRenderVisibleElements
          // Skip RF's measurement pass when a node has no width/height set
          // yet. Cheap on initial render with many nodes.
          nodeOrigin={[0, 0]}
          deleteKeyCode={["Delete", "Backspace"]}
          // Mouse: LEFT-drag pans the pane; RIGHT-drag marquee-selects (handled
          // by the wrapper's pointer handlers + getIntersectingNodes, since RF's
          // selectionOnDrag is left-button-only).
          panOnDrag={[0]}
          selectionMode={SelectionMode.Partial}
          multiSelectionKeyCode={["Shift", "Meta", "Control"]}
          // Disable RF's own Shift rubber-band: its default selectionKeyCode is
          // "Shift", which puts RF into box-select mode on Shift-press and lets
          // its d3 layer swallow the pointer events. We marquee-select via the
          // custom right-drag instead.
          selectionKeyCode={null}
          // NB: elements stay selectable (interactive) so edges/nodes remain
          // clickable; we instead drop RF's `select` changes in onNodes/Edges
          // Change so the document-level handler is the single selection
          // authority. (elementsSelectable={false} would also kill edge
          // pointer-events → unclickable edges.)
          // Treat any mouse movement under 4px as a click — fixes occasional missed
          // selects when the cursor wobbles a pixel between mousedown and mouseup.
          nodeDragThreshold={4}
          // Wheel still scrolls/zooms; that doesn't change.
          panOnScroll={false}
          panOnScrollMode={PanOnScrollMode.Free}
          proOptions={{ hideAttribution: true }}
        >
          {/* Dot grid. `--border` (L≈18% dark / 88% light) gives real contrast vs the
              canvas (`--background` L≈6%/99%); `--secondary` was too close to the bg to
              see. Theme-adaptive either way. */}
          <Background color="hsl(var(--border))" gap={GRID_GAP} />
          {/* Overview map, bottom-right. Dots colored by kind so the layout is
              readable at a glance: folders (have children) accent-blue, ghost
              cross-folder stubs dim, plain components gray. Click/drag the map
              to jump the viewport. */}
          <MiniMap
            position="bottom-right"
            pannable
            zoomable
            ariaLabel="Graph overview"
            style={{
              backgroundColor: "hsl(var(--border))",
              border: "1px solid hsl(var(--input))",
              borderRadius: 6,
            }}
            maskColor="hsl(var(--background) / 0.45)"
            nodeStrokeWidth={2}
            nodeColor={miniMapNodeColor}
            nodeStrokeColor={miniMapNodeStroke}
          />
          <ZoomRateController enabled={autoRate} setRate={wsAdapter.setRate} />
          <VisibilitySub onVisible={onVisibleSubscription} />
        </ReactFlow>
      </div>
      {/* Old LeftDock palette removed — replaced by the new left-drawer
          LeftPalette (top-left tab). FindPanel (Cmd/Ctrl+F) still does search. */}
      {clickDebugOpen && <ClickDebugger />}
      <DiagDrawer
        open={diagOpen}
        onClose={() => setDiagOpen(false)}
        bottomOffset={bottomBarH}
        wsRef={wsAdapter}
        autoRate={autoRate}
        manualRate={manualRate}
        onSetManualRate={onSetManualRate}
        onToggleAutoRate={() => setAutoRate((v) => !v)}
      />
      <PresenceBar />
      <FindPanel
        open={findOpen}
        currentParentUid={currentParentUid}
        onClose={() => setFindOpen(false)}
        onPick={(uid) => void goToComponent(uid)}
      />
      {marqueeRect &&
        // Portal to <body> so the fixed-position rect tracks the viewport cursor
        // even when an ancestor (rbx's shell) establishes a containing block via
        // transform/filter/contain — same reason the menus below portal out.
        createPortal(
          <div
            style={{
              position: "fixed",
              left: marqueeRect.x,
              top: marqueeRect.y,
              width: marqueeRect.w,
              height: marqueeRect.h,
              border: "1px solid hsl(var(--cool))",
              background: "rgba(74,158,255,0.12)",
              zIndex: 40,
              pointerEvents: "none",
            }}
          />,
          wiresheetPortalRoot(),
        )}
      {/* Node menu + its pickers / Configure panel are portaled to <body> so a
          right-click in the TABLE pane (outside the graph pane's clip) opens them
          at the cursor; modals also cover the full viewport. */}
      {createPortal(
        <>
          {nodeMenu && !movePickerOpen && !actionPickerOpen && detailsUid === null && (
            <NodeContextMenu
              x={nodeMenu.x}
              y={nodeMenu.y}
          hasActions={
            getActionsFor(nodes.filter((n) => n.selected).map((n) => Number(n.id))).length > 0
          }
          canRename={nodes.filter((n) => n.selected).length === 1}
          count={nodes.filter((n) => n.selected).length}
          uid={
            nodes.filter((n) => n.selected).length === 1
              ? Number(nodes.filter((n) => n.selected)[0].id)
              : undefined
          }
          name={
            nodes.filter((n) => n.selected).length === 1
              ? useStructural.getState().components.get(Number(nodes.filter((n) => n.selected)[0].id))
                  ?.name
              : undefined
          }
          path={
            nodes.filter((n) => n.selected).length === 1
              ? useStructural.getState().components.get(Number(nodes.filter((n) => n.selected)[0].id))
                  ?.path
              : undefined
          }
          onRename={async () => {
            const sel = nodes.filter((n) => n.selected).map((n) => Number(n.id));
            setNodeMenu(null);
            if (sel.length !== 1) return;
            const uid = sel[0];
            const cur = useStructural.getState().components.get(uid);
            const next = window.prompt("Rename component", cur?.name ?? "");
            if (next == null) return;
            const trimmed = next.trim();
            if (!trimmed || trimmed === cur?.name) return;
            try {
              await updateNode(uid, { name: trimmed });
              await reload();
            } catch (e) {
              reportError(e);
            }
          }}
          onDetails={() => {
            const sel = nodes.filter((n) => n.selected).map((n) => Number(n.id));
            if (sel.length === 1) setDetailsUid(sel[0]);
          }}
          uxAvailable={(() => {
            const sel = nodes.filter((n) => n.selected);
            if (sel.length !== 1) return false;
            const type = useStructural.getState().components.get(Number(sel[0].id))?.type;
            return !!findComponentUx(extensions, type);
          })()}
          onOpenUx={() => {
            const sel = nodes.filter((n) => n.selected);
            setNodeMenu(null);
            if (sel.length === 1) openComponentUx(Number(sel[0].id));
          }}
          onInspect={() => {
            const sel = nodes.filter((n) => n.selected);
            setNodeMenu(null);
            if (sel.length === 1) openComponentInspect(Number(sel[0].id));
          }}
          onGroup={() => {
            void groupSelected(nodes.filter((n) => n.selected).map((n) => Number(n.id)));
            setNodeMenu(null);
          }}
          onMoveInto={() => setMovePickerOpen(true)}
          onAction={() => setActionPickerOpen(true)}
          onClose={() => setNodeMenu(null)}
        />
      )}
      {nodeMenu && actionPickerOpen && (
        <ActionPicker
          x={nodeMenu.x}
          y={nodeMenu.y}
          targetUids={nodes.filter((n) => n.selected).map((n) => Number(n.id))}
          actions={getActionsFor(nodes.filter((n) => n.selected).map((n) => Number(n.id)))}
          onInvoke={invokeAction}
          onClose={() => {
            setActionPickerOpen(false);
            setNodeMenu(null);
          }}
        />
      )}
      {nodeMenu && movePickerOpen && (
        <MoveIntoPicker
          x={nodeMenu.x}
          y={nodeMenu.y}
          // Move every selected node (the right-click handler already ensured
          // the right-clicked node is in the selection). Drop the moving nodes
          // themselves from the candidate list — can't reparent into self.
          movingUids={
            nodes.filter((n) => n.selected).map((n) => Number(n.id))
          }
          onMove={async (newParent) => {
            const moving = nodes.filter((n) => n.selected).map((n) => Number(n.id));
            for (const uid of moving) {
              try {
                // Reparent — the maintainer reconciles the old + new ancestor
                // folders' exposure server-side (EXPOSURE_SPEC §5.1).
                await updateNode(uid, { parentUid: newParent });
              } catch (e) {
                reportError(e);
              }
            }
            setMovePickerOpen(false);
            setNodeMenu(null);
            // Reload — the moved components leave the current view (or stay if
            // we moved into a sibling of their old parent... no, sibling moves
            // also exit the view since we render only direct children).
            await reload();
          }}
          onClose={() => {
            setMovePickerOpen(false);
            setNodeMenu(null);
          }}
        />
      )}
      {detailsUid != null && (
        <ConfigurePanel
          componentUid={detailsUid}
          currentParentUid={currentParentUid}
          exposeProp={exposeProp}
          unexposeProp={unexposeProp}
          onSave={async (facetString) => {
            try {
              await updateNode(detailsUid, {
                properties: { [FACET_PROP]: { value: facetString } },
              });
              await reload();
            } catch (e) {
              reportError(e);
            }
          }}
          onClose={() => {
            setDetailsUid(null);
            setNodeMenu(null);
          }}
        />
      )}
        </>,
        wiresheetPortalRoot(),
      )}
      {/* Portal to <body> so the fixed-position menu tracks the viewport cursor
          even when an ancestor (rbx's shell) establishes a containing block via
          transform/filter/contain — same as the node menu above. */}
      {createPortal(
        paneMenu ? (
          <PaneContextMenu
            x={paneMenu.x}
            y={paneMenu.y}
            canGoUp={crumbs.length > 1}
            parentName={crumbs.length > 1 ? crumbs[crumbs.length - 2].name : ""}
            palette={palette}
            canPaste={(clipboardRef.current?.uids.length ?? 0) > 0}
            onUp={() => goToCrumb(crumbs.length - 2)}
            onAdd={(type) =>
              void onAddNode(type, rf.screenToFlowPosition({ x: paneMenu.x, y: paneMenu.y }))
            }
            onPaste={() => {
              mouseScreenPos.current = { x: paneMenu.x, y: paneMenu.y };
              void pasteFromClipboard();
            }}
            onClose={() => setPaneMenu(null)}
          />
        ) : null,
        wiresheetPortalRoot(),
      )}
      {createPortal(!edgeMenu ? null : (() => {
        // Look up loopback state from REST (source of truth). Determines which
        // primary action the menu offers. If the right-clicked edge can't be
        // found (e.g. just got removed under us), suppress the menu entirely.
        const rest = useStructural.getState().edges.get(Number(edgeMenu.edgeId));
        if (!rest) return null;
        const isLoop = rest.loopBack === true;
        return (
          <EdgeContextMenu
            x={edgeMenu.x}
            y={edgeMenu.y}
            isLoopBack={isLoop}
            onPrimary={() => {
              const ids = selectedEdgeIds(edges, edgeMenu.edgeId);
              // Filter selection to the same kind as the right-clicked edge so
              // a mixed selection doesn't accidentally apply the wrong action.
              const filtered = ids.filter((id) => {
                const e = useStructural.getState().edges.get(Number(id));
                return e ? (e.loopBack === true) === isLoop : false;
              });
              if (isLoop) void reEvaluateEdges(filtered.map(Number));
              else void setEdgesLoopBack(filtered.map(Number));
              setEdgeMenu(null);
            }}
            onDelete={() => {
              const ids = selectedEdgeIds(edges, edgeMenu.edgeId);
              const drop = edges.filter((e) => ids.includes(e.id));
              // Explicit edge delete — no component is being removed, so don't
              // let a prior gesture's node set cause a cascade-skip.
              deletingNodeIds.current = new Set();
              void onEdgesDelete(drop);
              setEdgeMenu(null);
            }}
            onClose={() => setEdgeMenu(null)}
          />
        );
      })(), wiresheetPortalRoot())}
      {error && <ErrorBanner error={error} onClose={() => setError(null)} />}
        {/* Left-edge palette tab + overlay (mirrors the right extension strip):
            a small drawer-icon tab toggles the palette, which floats OVER the
            canvas from the left edge — it never shifts the wiresheet. */}
        <div
          style={{
            position: "absolute",
            top: "50%",
            // Sit at the canvas edge when closed; ride to the palette's right edge
            // (w-60 = 240px) when open, so the tab stays attached to the drawer.
            left: paletteOpen ? 240 : 0,
            transform: "translateY(-50%)",
            zIndex: 22,
            transition: "left 180ms cubic-bezier(0.22, 1, 0.36, 1)",
          }}
        >
          {/* Matches the right ExtensionStrip edge tab (30×42, rounded on the
              canvas-facing edge, same surface colors). */}
          <button
            type="button"
            onClick={() => setPaletteOpen((v) => !v)}
            title={paletteOpen ? "Hide palette" : "Component palette"}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              width: 30,
              height: 42,
              border: "1px solid hsl(var(--border))",
              borderLeft: "none",
              borderRadius: "0 8px 8px 0",
              background: paletteOpen ? "hsl(var(--secondary))" : "hsl(var(--card) / 0.95)",
              color: paletteOpen ? "hsl(var(--foreground))" : "hsl(var(--muted-foreground))",
              // Blue active-accent on the inner (canvas-facing) edge — mirrors the
              // right strip's `inset 2px 0 0` (positive x → left; ours is right).
              boxShadow: paletteOpen ? "inset -2px 0 0 var(--ce-accent, hsl(var(--cool)))" : "none",
              cursor: "pointer",
            }}
          >
            <PanelLeft size={16} />
          </button>
        </div>
        {/* Kept mounted and slid in/out with translateX so it animates at the
            SAME speed/easing as the edge tab's `left` (which rides to 240px). */}
        <div
          style={{
            position: "absolute",
            left: 0,
            top: 0,
            bottom: 0,
            zIndex: 21,
            transform: paletteOpen ? "translateX(0)" : "translateX(-100%)",
            transition: "transform 180ms cubic-bezier(0.22, 1, 0.36, 1)",
            pointerEvents: paletteOpen ? "auto" : "none",
          }}
        >
          <LeftPalette
            open={paletteOpen}
            extensions={palette}
            onClose={() => setPaletteOpen(false)}
            onAdd={(type) =>
              void onAddNode(type, rf.screenToFlowPosition({ x: window.innerWidth / 2, y: window.innerHeight / 2 }))
            }
          />
        </div>
        {/* Right-edge tabs (one per extension). Sit at the screen edge when the
            drawer is closed, and at the drawer's left edge (the pane seam) when
            open — so they both open the drawer and switch the active extension. */}
        {extensions.length > 0 && (
          <div
            style={{
              position: "absolute",
              top: "50%",
              right: 0,
              transform: "translateY(-50%)",
              zIndex: 20,
            }}
          >
            <ExtensionStrip
              extensions={extensions}
              activeId={tableOpen ? activeExtId : null}
              onSelect={openExtension}
              variant="edge"
            />
          </div>
        )}
        </div>
      {drawerVisible && (
        <div
          onPointerDown={startSplitDrag}
          title="Drag to resize"
          style={{ width: 5, flexShrink: 0, cursor: "col-resize", background: "hsl(var(--border))" }}
        />
      )}
      {drawerVisible && (
        <div
          style={{
            flex: 1,
            minWidth: 0,
            height: "100%",
            display: "flex",
            flexDirection: "column",
            overflow: "hidden",
          }}
        >
          <div
            style={{
              display: "flex",
              alignItems: "center",
              gap: 4,
              padding: "3px 6px",
              background: "hsl(var(--card))",
              borderBottom: "1px solid hsl(var(--border))",
              flexShrink: 0,
            }}
          >
            <span style={{ marginRight: "auto", paddingLeft: 4, fontSize: 12, fontWeight: 600, color: "hsl(var(--foreground))" }}>
              {activeExt?.label ?? ""}
            </span>
            <button
              title={tableMaxed ? "Restore split" : "Maximize table"}
              onClick={() => {
                if (tableMaxed) setSplitPct(splitRestore.current || 55);
                else {
                  splitRestore.current = splitPct;
                  setSplitPct(0); // fill the whole wiresheet area (canvas collapses to 0%)
                }
              }}
              style={tableChromeBtn}
            >
              {tableMaxed ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
            </button>
            <button title="Close table" onClick={() => setTableOpen(false)} style={tableChromeBtn}>
              <X size={14} />
            </button>
          </div>
          <div style={{ flex: 1, minHeight: 0 }}>
            {activeExt && (
                <UiTabHost
                  uis={activeExt.uis}
                  focusRequest={uxFocus}
                  currentParentUid={currentParentUid}
                  selectedUids={tableSelected}
                  onSelect={onTableSelect}
                  onDrillIn={enter}
                  onNameContextMenu={openNodeContextMenu}
                  onRowsChange={onTableRows}
                  onAddComponent={onAddComponentByHint}
                  canGoUp={crumbs.length > 1}
                  onUp={() => goToCrumb(crumbs.length - 2)}
                  onSetDefault={onTableSetDefault}
                  onSetOverride={onTableSetOverride}
                  onClearOverride={onTableClearOverride}
                  onCallAction={async (uid, name, params) => {
                    try {
                      const r = await callAction(uid, name, params);
                      return r.returns;
                    } catch (e) {
                      reportError(e);
                      return {};
                    }
                  }}
                  onSubscribeProps={subscribeProps}
                  onLocate={(uid) => void goToComponent(uid)}
                />
            )}
          </div>
        </div>
      )}
        </div>
        {/* Bottom bar removed (matches rbx) — the breadcrumb moved to the top bar
            and the connection-status/diagnostics handle is in the top bar too. */}
      </div>
    </CeWiresheetContext.Provider>
  );
}

// If multiple edges are selected, act on the whole selection; otherwise act on
// just the right-clicked edge.
function selectedEdgeIds(edges: RfEdge[], rightClickedId: string): string[] {
  const sel = edges.filter((e) => e.selected).map((e) => e.id);
  return sel.length > 1 && sel.includes(rightClickedId) ? sel : [rightClickedId];
}

// Parse the Details panel's alias text field ("0=off, 1=auto, 2=manual") into
// {code,label} entries; ignores blanks / malformed parts.
const tableChromeBtn: CSSProperties = {
  display: "flex",
  alignItems: "center",
  background: "transparent",
  border: "none",
  color: "hsl(var(--muted-foreground))",
  cursor: "pointer",
  padding: "2px 4px",
};
