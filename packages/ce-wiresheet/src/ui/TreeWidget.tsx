// The `tree` widget — the folder hierarchy as an expandable tree with a
// component-count badge per folder, for a structural overview of the graph.
// Selection is shared with the host (sync): click selects, double-click drills
// in. Built from the store's parent relationships; counts use childrenCount so a
// folder shows its size even before its children are loaded.

import { useCallback, useEffect, useMemo, useState } from "react";
import { useStructural } from "../lib/store";
import { getNodeByUid } from "../lib/rest";
import { ChevronRight, ChevronDown, Folder, FolderOpen, Dot } from "lucide-react";
import type { Component } from "../lib/engine-types";

export interface TreeWidgetProps {
  currentParentUid: number;
  selectedUids: number[];
  onSelect?: (uids: number[]) => void;
  onDrillIn?: (uid: number) => void;
  /** Navigate the canvas to a component (drill into its folder + center + select).
   *  Used for click-to-go-there on deep rows the current view doesn't contain. */
  onLocate?: (uid: number) => void;
}

export function TreeWidget({ currentParentUid, selectedUids, onSelect, onDrillIn, onLocate }: TreeWidgetProps) {
  const components = useStructural((s) => s.components);
  // Everything collapsed by default; the root folder starts open so the first
  // level is visible. Reset when the current folder changes (navigation).
  const [expanded, setExpanded] = useState<Set<number>>(() => new Set([currentParentUid]));
  // Children of folders OUTSIDE the current view aren't in the structural store
  // (it only holds the open folder's level). Lazily fetched per folder on expand
  // so the tree can drill arbitrarily deep without loading the whole graph.
  const [fetched, setFetched] = useState<Map<number, Component[]>>(new Map());
  useEffect(() => { setExpanded(new Set([currentParentUid])); setFetched(new Map()); }, [currentParentUid]);

  const childrenByParent = useMemo(() => {
    const m = new Map<number, Component[]>();
    for (const c of components.values()) {
      const arr = m.get(c.parent);
      if (arr) arr.push(c);
      else m.set(c.parent, [c]);
    }
    // Folders (components with children) first, then leaves — each alphabetical.
    const isFolder = (c: Component) => (c.childrenCount ?? 0) > 0 || (m.get(c.uid)?.length ?? 0) > 0;
    for (const arr of m.values())
      arr.sort((a, b) => (isFolder(b) ? 1 : 0) - (isFolder(a) ? 1 : 0) || a.name.localeCompare(b.name));
    return m;
  }, [components]);

  // A folder's children, from the store (current level) or the lazy cache (deeper).
  const getKids = useCallback(
    (uid: number): Component[] => {
      const stored = childrenByParent.get(uid);
      if (stored && stored.length) return stored;
      return fetched.get(uid) ?? [];
    },
    [childrenByParent, fetched],
  );

  // Fetch a folder's direct children once (on first expand) when they aren't loaded.
  const loadChildren = useCallback(
    async (uid: number) => {
      if ((childrenByParent.get(uid)?.length ?? 0) > 0 || fetched.has(uid)) return;
      try {
        const resp = await getNodeByUid(uid, { depth: 1, nested: true });
        const kids = resp.nodes[0]?.children ?? [];
        setFetched((m) => (m.has(uid) ? m : new Map(m).set(uid, kids)));
      } catch { /* leave the folder showing empty */ }
    },
    [childrenByParent, fetched],
  );

  // Total components contained under a folder (recursively, over loaded data).
  const totalUnder = useMemo(() => {
    const memo = new Map<number, number>();
    const count = (uid: number): number => {
      if (memo.has(uid)) return memo.get(uid)!;
      let n = 0;
      for (const c of getKids(uid)) n += 1 + count(c.uid);
      memo.set(uid, n);
      return n;
    };
    return count;
  }, [getKids]);

  const sel = new Set(selectedUids);
  const toggle = (uid: number) => {
    if (!expanded.has(uid)) void loadChildren(uid); // opening → fetch children if needed
    setExpanded((s) => {
      const n = new Set(s);
      n.has(uid) ? n.delete(uid) : n.add(uid);
      return n;
    });
  };

  const renderRow = (
    uid: number,
    name: string,
    depth: number,
    isFolder: boolean,
    count: number,
    selectable: boolean,
  ): React.ReactNode => {
    const open = isFolder && expanded.has(uid);
    const kids = getKids(uid);
    return (
      <div key={uid}>
        <div
          onClick={
            selectable
              ? () => {
                  // Depth 1 is the open folder's own level — already on the canvas,
                  // so a plain select. Deeper rows aren't in the current view, so
                  // navigate to them (drill into their folder + center + select).
                  if (depth === 1 && onSelect) onSelect([uid]);
                  else if (onLocate) onLocate(uid);
                  else onSelect?.([uid]);
                }
              : isFolder
                ? () => toggle(uid)
                : undefined
          }
          onDoubleClick={selectable ? () => onDrillIn?.(uid) : undefined}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 4,
            padding: "3px 6px",
            paddingLeft: 6 + depth * 14,
            cursor: isFolder || selectable ? "pointer" : "default",
            background: sel.has(uid) ? "hsl(var(--cool) / 0.18)" : "transparent",
            fontSize: 12,
            whiteSpace: "nowrap",
          }}
        >
          {isFolder ? (
            <button onClick={(e) => { e.stopPropagation(); toggle(uid); }} style={chevBtn} title={open ? "Collapse" : "Expand"}>
              {open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
            </button>
          ) : (
            <span style={{ width: 15, flexShrink: 0 }} />
          )}
          {isFolder ? (
            open ? <FolderOpen size={13} color="hsl(var(--cool))" /> : <Folder size={13} color="hsl(var(--cool))" />
          ) : (
            <Dot size={13} color="hsl(var(--muted-foreground))" />
          )}
          <span style={{ color: "hsl(var(--foreground))" }}>{name}</span>
          {isFolder && <span style={countBadge}>{count}</span>}
        </div>
        {open && kids.map((k) => {
          const kf = (k.childrenCount ?? 0) > 0 || getKids(k.uid).length > 0;
          return renderRow(k.uid, k.name, depth + 1, kf, totalUnder(k.uid) || (k.childrenCount ?? 0), true);
        })}
      </div>
    );
  };

  const rootComp = components.get(currentParentUid);
  const rootName = rootComp?.name ?? "root";
  const rootCount = totalUnder(currentParentUid) || (rootComp?.childrenCount ?? 0);
  const hasAny = (childrenByParent.get(currentParentUid)?.length ?? 0) > 0;

  return (
    <div style={{ height: "100%", overflow: "auto", userSelect: "none" }}>
      {!hasAny ? (
        <div style={{ padding: 12, color: "hsl(var(--muted-foreground))", fontSize: 12 }}>no components in this folder</div>
      ) : (
        renderRow(currentParentUid, rootName, 0, true, rootCount, false)
      )}
    </div>
  );
}

const chevBtn: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  width: 15,
  height: 15,
  border: "none",
  background: "transparent",
  color: "hsl(var(--muted-foreground))",
  cursor: "pointer",
  padding: 0,
  flexShrink: 0,
};
const countBadge: React.CSSProperties = {
  marginLeft: 6,
  fontSize: 9,
  color: "hsl(var(--muted-foreground))",
  background: "hsl(var(--secondary))",
  borderRadius: 8,
  padding: "0 6px",
  lineHeight: "14px",
};
