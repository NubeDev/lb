import { useEffect, useRef, useState } from "react";
import { EdgeMenuItem } from "./EdgeMenuItem";
import { acInput, acBtn } from "./styles";
import type { PaletteExtension } from "./types";

// Right-click menu on empty canvas: go up, add a component (with a filterable,
// keyboard-navigable picker), or paste.
export function PaneContextMenu({
  x,
  y,
  canGoUp,
  parentName,
  palette,
  canPaste,
  onUp,
  onAdd,
  onPaste,
  onClose,
}: {
  x: number;
  y: number;
  canGoUp: boolean;
  parentName: string;
  palette: PaletteExtension[];
  canPaste: boolean;
  onUp: () => void;
  onAdd: (type: string) => void;
  onPaste: () => void;
  onClose: () => void;
}) {
  const [adding, setAdding] = useState(false);
  const [filter, setFilter] = useState("");
  const [highlight, setHighlight] = useState(0);
  const hlRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    setHighlight(0);
  }, [filter, adding]);
  useEffect(() => {
    hlRef.current?.scrollIntoView({ block: "nearest" });
  }, [highlight]);

  useEffect(() => {
    const dismiss = (e: MouseEvent) => {
      const el = e.target as Element | null;
      if (el && el.closest("[data-ce-node-menu]")) return;
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

  const W = adding ? 240 : 180;
  const left = Math.min(x, window.innerWidth - W - 8);
  const top = Math.min(y, window.innerHeight - (adding ? 320 : 140));

  const all = palette.flatMap((g) =>
    g.components.map((c) => ({ name: c.name, type: c.type, group: g.id })),
  );
  const f = filter.trim().toLowerCase();
  const filtered = f
    ? all.filter((c) => c.name.toLowerCase().includes(f) || c.type.toLowerCase().includes(f))
    : all;

  return (
    <div
      data-ce-node-menu
      onContextMenu={(e) => e.preventDefault()}
      style={{
        position: "fixed",
        left,
        top,
        zIndex: 100,
        background: "hsl(var(--card))",
        border: "1px solid hsl(var(--border))",
        borderRadius: 4,
        width: W,
        maxHeight: adding ? 320 : undefined,
        boxShadow: "0 4px 12px rgba(0,0,0,0.5)",
        fontSize: 12,
        color: "hsl(var(--foreground))",
        fontFamily: "-apple-system, system-ui, sans-serif",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
      }}
    >
      {adding ? (
        <>
          <div
            style={{
              padding: "6px 8px",
              borderBottom: "1px solid hsl(var(--border))",
              display: "flex",
              alignItems: "center",
              gap: 6,
            }}
          >
            <button
              onClick={() => setAdding(false)}
              title="Back"
              style={{
                background: "transparent",
                border: "none",
                color: "hsl(var(--cool))",
                cursor: "pointer",
                fontSize: 14,
                padding: 0,
              }}
            >
              ‹
            </button>
            <input
              autoFocus
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  onClose();
                  return;
                }
                if (e.key === "ArrowDown") {
                  e.preventDefault();
                  setHighlight((h) => Math.min(h + 1, Math.max(0, filtered.length - 1)));
                  return;
                }
                if (e.key === "ArrowUp") {
                  e.preventDefault();
                  setHighlight((h) => Math.max(0, h - 1));
                  return;
                }
                if (e.key === "Enter") {
                  e.preventDefault();
                  const c = filtered[highlight];
                  if (c) {
                    onAdd(c.type);
                    onClose();
                  }
                  return;
                }
                e.stopPropagation();
              }}
              placeholder="Filter components…"
              style={{ ...acInput, flex: 1 }}
            />
          </div>
          <div style={{ overflowY: "auto", padding: 4 }}>
            {filtered.length === 0 ? (
              <div style={{ color: "hsl(var(--muted-foreground))", padding: "6px 8px" }}>no matches</div>
            ) : (
              filtered.map((c, i) => (
                <button
                  key={c.type}
                  ref={i === highlight ? hlRef : undefined}
                  onMouseEnter={() => setHighlight(i)}
                  onClick={() => {
                    onAdd(c.type);
                    onClose();
                  }}
                  style={{ ...acBtn, background: i === highlight ? "hsl(var(--cool) / 0.18)" : "transparent" }}
                >
                  <span>{c.name}</span>
                  <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>{c.group}</span>
                </button>
              ))
            )}
          </div>
        </>
      ) : (
        <div style={{ padding: 4 }}>
          {canGoUp && (
            <EdgeMenuItem
              label={`‹ Up to ${parentName}`}
              onClick={() => {
                onUp();
                onClose();
              }}
            />
          )}
          <EdgeMenuItem label="Add component…" onClick={() => setAdding(true)} />
          {canPaste && (
            <EdgeMenuItem
              label="Paste"
              onClick={() => {
                onPaste();
                onClose();
              }}
            />
          )}
        </div>
      )}
    </div>
  );
}
