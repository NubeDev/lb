import { useEffect, useState } from "react";
import type { Component } from "../../lib/engine-types";
import { getRootNodes } from "../../lib/rest";
import { useStructural } from "../../lib/store";
import { moveCandidates, filterMoveCandidates } from "../../lib/movepicker";

// Reparent picker: choose a destination component. Fetches the full tree so
// up-the-tree moves are reachable, then tiers candidates (up one level → same
// level → children → elsewhere) via lib/movepicker.
export function MoveIntoPicker({
  x,
  y,
  movingUids,
  onMove,
  onClose,
}: {
  x: number;
  y: number;
  movingUids: number[];
  onMove: (newParentUid: number) => void | Promise<void>;
  onClose: () => void;
}) {
  const [filter, setFilter] = useState("");
  // useStructural only holds the current view's children, so up-the-tree
  // moves (back to root, into an ancestor folder) wouldn't be reachable
  // without a full-tree fetch. Same pattern as ConnectPicker.
  const [allComponents, setAllComponents] = useState<Component[] | null>(null);
  const movingSet = new Set(movingUids);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const resp = await getRootNodes({ depth: -1, nested: true });
        if (cancelled) return;
        const flat: Component[] = [];
        const walk = (c: Component) => {
          flat.push(c);
          c.children?.forEach(walk);
        };
        // Include root itself in the candidate list — it's a legitimate
        // destination (move out of any folder back to the top level).
        resp.nodes.forEach(walk);
        setAllComponents(flat);
      } catch {
        if (cancelled) return;
        setAllComponents([...useStructural.getState().components.values()]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Candidate destinations (self/descendant exclusion + tiering) live in
  // lib/movepicker (tested): up one level → same level → children → elsewhere.
  const candidates = moveCandidates(allComponents ?? [], movingSet);
  const visible = filterMoveCandidates(candidates, filter);

  useEffect(() => {
    const dismiss = (e: MouseEvent) => {
      const el = e.target as Element | null;
      if (el && el.closest("[data-ce-node-menu]")) return;
      onClose();
    };
    // Capture phase + pointerdown: React Flow's pane (d3-zoom) calls
    // stopImmediatePropagation on pointer/mouse down, so a bubble-phase
    // document listener never sees outside clicks. Capture fires first.
    document.addEventListener("pointerdown", dismiss, true);
    document.addEventListener("contextmenu", dismiss, true);
    return () => {
      document.removeEventListener("pointerdown", dismiss, true);
      document.removeEventListener("contextmenu", dismiss, true);
    };
  }, [onClose]);

  const PICKER_W = 260;
  const left = Math.min(x, window.innerWidth - PICKER_W - 8);
  const top = Math.min(y, window.innerHeight - 320);
  return (
    <div
      data-ce-node-menu
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
        fontSize: 12,
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div style={{ padding: "6px 8px", borderBottom: "1px solid hsl(var(--border))" }}>
        <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, marginBottom: 4 }}>
          Move {movingUids.length === 1 ? "1 component" : `${movingUids.length} components`} into…
        </div>
        <input
          autoFocus
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            else if (e.key === "Enter" && visible.length === 1) onMove(visible[0].uid);
            e.stopPropagation();
          }}
          placeholder="filter…"
          style={{
            width: "100%",
            background: "hsl(var(--background))",
            color: "hsl(var(--foreground))",
            border: "1px solid hsl(var(--border))",
            borderRadius: 2,
            padding: "3px 6px",
            fontSize: 12,
            fontFamily: "var(--font-mono)",
            boxSizing: "border-box",
            outline: "none",
          }}
        />
      </div>
      <div style={{ flex: 1, overflowY: "auto" }}>
        {visible.length === 0 ? (
          <div style={{ padding: "10px 8px", color: "hsl(var(--muted-foreground))", fontSize: 12 }}>
            {allComponents == null ? "loading…" : "no destinations"}
          </div>
        ) : (
          visible.map((c, idx) => {
            // Drop the leading "root/" from the displayed path so the column
            // reads cleanly; bare "root" shows as the explicit top-level
            // option.
            const pathLabel =
              c.path === "root" ? "root" : c.path.startsWith("root/") ? c.path.slice(5) : c.path;
            const showSection = c.tier !== (idx > 0 ? visible[idx - 1].tier : -1);
            const sectionLabel =
              c.tier === 0
                ? "up one level"
                : c.tier === 1
                  ? "same level"
                  : c.tier === 2
                    ? "inside this folder"
                    : "other";
            return (
              <div key={c.uid}>
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
                  onClick={() => onMove(c.uid)}
                style={{
                  display: "flex",
                  width: "100%",
                  textAlign: "left",
                  padding: "5px 8px",
                  background: "transparent",
                  color: "hsl(var(--foreground))",
                  border: "none",
                  cursor: "pointer",
                  fontSize: 12,
                  fontFamily: "var(--font-mono)",
                  alignItems: "baseline",
                  gap: 6,
                }}
                onMouseEnter={(e) => (e.currentTarget.style.background = "hsl(var(--border))")}
                onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
              >
                <span
                  style={{
                    color: "hsl(var(--cool))",
                    flex: 1,
                    minWidth: 0,
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                  title={c.path}
                >
                  {pathLabel}
                </span>
                <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11, flexShrink: 0 }}>{c.kind}</span>
                </button>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
}
