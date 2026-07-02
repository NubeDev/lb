import { useEffect, useRef, useState } from "react";
import { useStructural } from "../lib/store";
import { metrics } from "../lib/instrumentation";

// Debug overlay for selection issues. Captures pointerdown/up at the document level
// (capture phase, BEFORE React Flow sees the event) and records:
//   - where the pointer went down
//   - what node (by name) it was on, if any
//   - how far it travelled before the up
//   - which mouse button
//   - whether React Flow ended up selecting anything as a result
//
// Two visualisations:
//   - a transient ring at the down point that fades over ~700ms
//   - a small fixed log of the last 8 events in the bottom-right

interface ClickEvent {
  id: number;
  x: number;
  y: number;
  button: number;
  nodeName: string | null;   // resolved component name, or null if not on a node
  nodeUid: number | null;
  targetTag: string;         // a short hint at what kind of element was clicked
  distance: number;
  duration: number;
  selectedAfter: number;
  ts: number;
}

const RING_LIFETIME_MS = 800;
const LOG_LIMIT = 8;

interface TargetInfo {
  nodeUid: number | null;
  targetTag: string;
}

// Walks UP from the clicked element until it finds a `.react-flow__node` (whose
// `data-id` is the component UID), or hits something interesting like a handle.
function describeTarget(el: Element | null): TargetInfo {
  if (!el) return { nodeUid: null, targetTag: "<none>" };
  // What kind of thing did we actually hit? Useful when the user clicks on a handle
  // edge dot vs the node body.
  let kind = "pane";
  let node: Element | null = el;
  let nodeEl: HTMLElement | null = null;
  while (node) {
    if (node.classList?.contains("react-flow__handle")) kind = "handle";
    if (node.classList?.contains("react-flow__node")) {
      nodeEl = node as HTMLElement;
      if (kind === "pane") kind = "node body";
      break;
    }
    if (node.classList?.contains("react-flow__pane")) kind = "pane";
    node = node.parentElement;
  }
  const uid = nodeEl?.dataset?.id ? Number(nodeEl.dataset.id) : null;
  return { nodeUid: Number.isFinite(uid as number) ? (uid as number) : null, targetTag: kind };
}

let nextId = 1;

export function ClickDebugger() {
  const [events, setEvents] = useState<ClickEvent[]>([]);
  const [rings, setRings] = useState<Array<{ id: number; x: number; y: number; ts: number }>>([]);
  const downRef = useRef<
    | { x: number; y: number; ts: number; button: number; nodeUid: number | null; targetTag: string }
    | null
  >(null);

  // Subscribe so a topology rename re-renders the panel and the log re-resolves names.
  useStructural((s) => s.components);

  // Live readout of the most recent React Flow select change (captured by App.tsx).
  // Ref-driven via rAF so we don't have to involve React state for high-frequency
  // updates. The header line shows "rf: addA=+ counter=-" when RF flipped selection.
  const rSelChange = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    let raf = 0;
    let lastTs = 0;
    const tick = () => {
      if (rSelChange.current && metrics.lastSelChangeAt !== lastTs) {
        lastTs = metrics.lastSelChangeAt;
        rSelChange.current.textContent = metrics.lastSelChange || "—";
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  useEffect(() => {
    const onDown = (e: PointerEvent) => {
      const id = nextId++;
      const info = describeTarget(e.target as Element);
      downRef.current = {
        x: e.clientX,
        y: e.clientY,
        ts: performance.now(),
        button: e.button,
        nodeUid: info.nodeUid,
        targetTag: info.targetTag,
      };
      setRings((r) => [...r, { id, x: e.clientX, y: e.clientY, ts: performance.now() }]);
      setTimeout(() => setRings((r) => r.filter((x) => x.id !== id)), RING_LIFETIME_MS);
    };
    const onUp = (e: PointerEvent) => {
      const d = downRef.current;
      if (!d) return;
      downRef.current = null;
      const dx = e.clientX - d.x;
      const dy = e.clientY - d.y;
      const dist = Math.sqrt(dx * dx + dy * dy);
      // After react flow has had a tick to react, look at the actual selected set
      setTimeout(() => {
        const selected = document.querySelectorAll(".react-flow__node.selected").length;
        const name =
          d.nodeUid != null
            ? useStructural.getState().components.get(d.nodeUid)?.name ?? null
            : null;
        const ev: ClickEvent = {
          id: nextId++,
          x: d.x,
          y: d.y,
          button: d.button,
          nodeName: name,
          nodeUid: d.nodeUid,
          targetTag: d.targetTag,
          distance: dist,
          duration: performance.now() - d.ts,
          selectedAfter: selected,
          ts: Date.now(),
        };
        setEvents((es) => [ev, ...es].slice(0, LOG_LIMIT));
      }, 60);
    };
    window.addEventListener("pointerdown", onDown, true);
    window.addEventListener("pointerup", onUp, true);
    return () => {
      window.removeEventListener("pointerdown", onDown, true);
      window.removeEventListener("pointerup", onUp, true);
    };
  }, []);

  return (
    <>
      {/* Click rings, drawn above the canvas */}
      <div style={{ position: "fixed", inset: 0, pointerEvents: "none", zIndex: 9998 }}>
        {rings.map((r) => (
          <div
            key={r.id}
            style={{
              position: "absolute",
              left: r.x - 14,
              top: r.y - 14,
              width: 28,
              height: 28,
              borderRadius: 14,
              border: "2px solid hsl(var(--cool))",
              animation: "ce-ring-fade 800ms ease-out forwards",
              boxSizing: "border-box",
            }}
          />
        ))}
      </div>

      {/* Event log, bottom-right above the corner */}
      <div
        style={{
          position: "fixed",
          right: 12,
          bottom: 12,
          zIndex: 30,
          background: "hsl(var(--card) / 0.92)",
          border: "1px solid hsl(var(--border))",
          borderRadius: 6,
          padding: 8,
          color: "hsl(var(--foreground))",
          fontSize: 10,
          fontFamily: "var(--font-mono)",
          minWidth: 320,
          maxWidth: 460,
        }}
      >
        <div style={{ color: "hsl(var(--muted-foreground))", marginBottom: 4, display: "flex", gap: 8 }}>
          <span>click debugger · last {LOG_LIMIT}</span>
          <span style={{ marginLeft: "auto", color: "hsl(var(--foreground))" }}>
            rf: <span ref={rSelChange}>—</span>
          </span>
        </div>
        {events.length === 0 && <div style={{ color: "hsl(var(--muted-foreground))" }}>click somewhere…</div>}
        {events.map((e) => {
          const buttonName = e.button === 0 ? "L" : e.button === 2 ? "R" : `M${e.button}`;
          const kind = e.distance < 4 ? "click" : "drag";
          const sel = e.selectedAfter > 0 ? `sel=${e.selectedAfter}` : "sel=0";
          const onWhat =
            e.nodeName != null
              ? `${e.nodeName}  [${e.targetTag}]`
              : `(${e.targetTag})`;
          return (
            <div key={e.id} style={{ display: "flex", gap: 6 }}>
              <span style={{ color: e.button === 2 ? "hsl(var(--amber))" : "hsl(var(--cool))", width: 14 }}>{buttonName}</span>
              <span style={{ width: 36 }}>{kind}</span>
              <span style={{ color: "hsl(var(--muted-foreground))", width: 70 }}>
                {e.distance.toFixed(0)}px · {e.duration.toFixed(0)}ms
              </span>
              <span style={{ color: e.selectedAfter > 0 ? "hsl(var(--green))" : "hsl(var(--crit))", width: 38 }}>{sel}</span>
              <span
                style={{
                  flex: 1,
                  minWidth: 0,
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                  color: e.nodeName ? "hsl(var(--foreground))" : "hsl(var(--muted-foreground))",
                }}
                title={e.nodeUid != null ? `uid ${e.nodeUid}` : undefined}
              >
                {onWhat}
              </span>
            </div>
          );
        })}
      </div>

      {/* keyframe for the fade */}
      <style>{`
        @keyframes ce-ring-fade {
          0%   { opacity: 1; transform: scale(0.7); }
          80%  { opacity: 0.7; transform: scale(1.6); }
          100% { opacity: 0;   transform: scale(2);   }
        }
      `}</style>
    </>
  );
}
