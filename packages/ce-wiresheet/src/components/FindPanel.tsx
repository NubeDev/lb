import { useEffect, useMemo, useRef, useState } from "react";
import type { Component } from "../lib/engine-types";
import { getRootNodes } from "../lib/rest";

// Command-palette-style component finder. Opens on Cmd/Ctrl+F (or a button),
// searches the WHOLE tree by name / type / path, and on select jumps the view
// to the component's folder and centers + selects it (via the same
// goToComponent path the cross-folder ghosts use).
//
// Fetches the full tree once per open so search spans folders the user isn't
// currently viewing — the whole point on a big flow.

interface Hit {
  uid: number;
  name: string;
  type: string;
  path: string; // stripped of leading "root/"
  parent: number;
  here: boolean; // in the folder currently being viewed
  props: { name: string; uid: number }[]; // this component's properties, for uid/name search
}

type ScoredHit = Hit & { matchedProp?: { name: string; uid: number } };

export function FindPanel({
  open,
  currentParentUid,
  onClose,
  onPick,
}: {
  open: boolean;
  currentParentUid: number;
  onClose: () => void;
  onPick: (uid: number) => void;
}) {
  const [query, setQuery] = useState("");
  const [all, setAll] = useState<Hit[] | null>(null);
  const [sel, setSel] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Load the full tree each time the panel opens (cheap on the scale we run at;
  // keeps results fresh after edits).
  useEffect(() => {
    if (!open) return;
    setQuery("");
    setSel(0);
    let cancelled = false;
    (async () => {
      try {
        const resp = await getRootNodes({ depth: -1, nested: true });
        if (cancelled) return;
        const flat: Hit[] = [];
        const walk = (c: Component) => {
          // Skip root itself; it's not a navigable target.
          if (c.uid !== 0) {
            const path = c.path.startsWith("root/") ? c.path.slice(5) : c.path;
            flat.push({
              uid: c.uid,
              name: c.name || c.type,
              type: c.type,
              path,
              parent: c.parent,
              here: c.parent === currentParentUid,
              props: Object.entries(c.properties ?? {}).map(([name, p]) => ({ name, uid: p.uid })),
            });
          }
          c.children?.forEach(walk);
        };
        resp.nodes.forEach(walk);
        setAll(flat);
      } catch {
        if (!cancelled) setAll([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, currentParentUid]);

  // Focus the input when opened.
  useEffect(() => {
    if (open) {
      // Defer so the element exists.
      const t = window.setTimeout(() => inputRef.current?.focus(), 0);
      return () => window.clearTimeout(t);
    }
  }, [open]);

  const results = useMemo<ScoredHit[]>(() => {
    if (!all) return [];
    const q = query.trim().toLowerCase();
    if (!q) return all.slice(0, 50);
    const scored = all
      .map((h) => {
        const name = h.name.toLowerCase();
        // Rank: exact uid/name > name prefix > name/uid contains > prop/path/type.
        // Component + prop UIDs are searchable too — handy when you have a uid from
        // a node tooltip, the API, or logs and want to jump straight to it.
        let score = -1;
        let matchedProp: { name: string; uid: number } | undefined;
        const propExact = h.props.find((p) => String(p.uid) === q);
        if (String(h.uid) === q) score = 0; // exact component uid
        else if (propExact) { score = 0; matchedProp = propExact; } // exact prop uid
        else if (name === q) score = 0;
        else if (name.startsWith(q)) score = 1;
        else if (name.includes(q)) score = 2;
        else if (String(h.uid).includes(q)) score = 2; // partial component uid
        else {
          const propPartial = h.props.find(
            (p) => String(p.uid).includes(q) || p.name.toLowerCase().includes(q),
          );
          if (propPartial) { score = 3; matchedProp = propPartial; } // prop uid/name
          else if (h.path.toLowerCase().includes(q) || h.type.toLowerCase().includes(q)) score = 3;
        }
        return { h, score, matchedProp };
      })
      .filter((x) => x.score >= 0)
      // Current-folder hits first, then by match score, then name. So "what's
      // on this level" floats to the top and is also badged in the row.
      .sort(
        (a, b) =>
          Number(b.h.here) - Number(a.h.here) ||
          a.score - b.score ||
          a.h.name.localeCompare(b.h.name),
      )
      .slice(0, 50)
      .map((x) => ({ ...x.h, matchedProp: x.matchedProp }));
    return scored;
  }, [all, query]);

  // Keep the selected row in view + clamp selection when results change.
  useEffect(() => {
    if (sel >= results.length) setSel(0);
  }, [results, sel]);
  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-idx="${sel}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [sel]);

  if (!open) return null;

  // (FindHeader defined at module scope below.)

  const pick = (h: Hit | undefined) => {
    if (!h) return;
    onPick(h.uid);
    onClose();
  };

  return (
    <div
      // Backdrop — click outside closes.
      onMouseDown={onClose}
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 200,
        background: "rgba(0,0,0,0.35)",
        display: "flex",
        justifyContent: "center",
        alignItems: "flex-start",
        paddingTop: "12vh",
      }}
    >
      <div
        onMouseDown={(e) => e.stopPropagation()}
        style={{
          width: 480,
          maxWidth: "90vw",
          background: "hsl(var(--card))",
          border: "1px solid hsl(var(--border))",
          borderRadius: 8,
          boxShadow: "0 12px 40px rgba(0,0,0,0.6)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          fontFamily: "-apple-system, system-ui, sans-serif",
        }}
      >
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSel(0);
          }}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              onClose();
            } else if (e.key === "ArrowDown") {
              e.preventDefault();
              setSel((s) => Math.min(results.length - 1, s + 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setSel((s) => Math.max(0, s - 1));
            } else if (e.key === "Enter") {
              e.preventDefault();
              pick(results[sel]);
            }
            e.stopPropagation();
          }}
          placeholder="Find by name, type, path, or uid (component / prop)…"
          spellCheck={false}
          style={{
            background: "hsl(var(--background))",
            color: "hsl(var(--foreground))",
            border: "none",
            borderBottom: "1px solid hsl(var(--border))",
            padding: "12px 14px",
            fontSize: 14,
            fontFamily: "var(--font-mono)",
            outline: "none",
          }}
        />
        <div ref={listRef} style={{ maxHeight: "50vh", overflowY: "auto" }}>
          {all == null ? (
            <div style={{ padding: "12px 14px", color: "hsl(var(--muted-foreground))", fontSize: 12 }}>loading…</div>
          ) : results.length === 0 ? (
            <div style={{ padding: "12px 14px", color: "hsl(var(--muted-foreground))", fontSize: 12 }}>
              no matches
            </div>
          ) : (
            results.map((h, i) => {
              // Section dividers: "this folder" above the first here-hit,
              // "elsewhere" at the here→other boundary.
              const prev = i > 0 ? results[i - 1] : null;
              const showHereHeader = h.here && (prev === null || !prev.here);
              const showElsewhereHeader = !h.here && (prev === null || prev.here);
              return (
                <div key={h.uid}>
                  {showHereHeader && <FindHeader label="this folder" />}
                  {showElsewhereHeader && <FindHeader label="elsewhere" />}
                  <button
                    data-idx={i}
                    onMouseEnter={() => setSel(i)}
                    onClick={() => pick(h)}
                    style={{
                      display: "flex",
                      width: "100%",
                      textAlign: "left",
                      alignItems: "baseline",
                      gap: 8,
                      padding: "8px 14px 8px 12px",
                      background: i === sel ? "hsl(var(--cool) / 0.18)" : "transparent",
                      border: "none",
                      // Left accent on same-folder rows so they read as "here"
                      // even mid-scroll, past the section header.
                      borderLeft: `2px solid ${h.here ? "hsl(var(--cool))" : "transparent"}`,
                      cursor: "pointer",
                      fontFamily: "var(--font-mono)",
                    }}
                  >
                    <span style={{ color: "hsl(var(--foreground))", fontSize: 13, flexShrink: 0 }}>{h.name}</span>
                    {/* When a component/prop UID drove the match, show it so the
                        reason is obvious (e.g. matched prop "in1 #1000108"). */}
                    {h.matchedProp ? (
                      <span style={{ color: "hsl(var(--cool))", fontSize: 11, flexShrink: 0 }}>
                        {h.matchedProp.name} #{h.matchedProp.uid}
                      </span>
                    ) : null}
                    <span
                      style={{
                        color: "hsl(var(--muted-foreground))",
                        fontSize: 11,
                        flex: 1,
                        minWidth: 0,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={`${h.path} · ${h.type} · uid ${h.uid}`}
                    >
                      {/* For same-folder hits the path is the current view, so
                          show the type instead of repeating the folder. */}
                      {h.here ? h.type : h.path}
                    </span>
                    <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, flexShrink: 0 }}>#{h.uid}</span>
                  </button>
                </div>
              );
            })
          )}
        </div>
        <div
          style={{
            padding: "6px 14px",
            borderTop: "1px solid hsl(var(--border))",
            color: "hsl(var(--muted-foreground))",
            fontSize: 10,
            fontFamily: "var(--font-mono)",
          }}
        >
          ↑↓ navigate · ↵ go · esc close
        </div>
      </div>
    </div>
  );
}

// Section header inside the results list ("this folder" / "elsewhere").
function FindHeader({ label }: { label: string }) {
  return (
    <div
      style={{
        padding: "6px 14px 3px 14px",
        color: "hsl(var(--muted-foreground))",
        fontSize: 9,
        textTransform: "uppercase",
        letterSpacing: 0.5,
        fontFamily: "var(--font-mono)",
      }}
    >
      {label}
    </div>
  );
}
