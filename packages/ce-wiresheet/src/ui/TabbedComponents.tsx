// `tabbedEditor` widget — a generic IDE-style tabbed editor for one component
// KIND. A pinned "All" tab lists every matching component (in-folder live +
// global `?type=` scan), and opening one becomes a closeable tab that renders
// the configured inner widget for that component.
//
// Descriptor config (on the node):
//   inner:      Widget to render per active component (its `type` is the KIND,
//               e.g. {type:"schedule", bind:{prop:"config"}, action:{…}})
//   fullType:   exact "vendor-ext::name" for the global scan (in-folder still
//               works without it; out-of-folder needs it)
//   indexLabel: noun for the index header (e.g. "schedules")
//
// Open tabs stay mounted (drafts survive switching); the host's prop
// subscription is single-owner, so each inner widget receives `ctx.active` and
// only the active one subscribes.

import { useCallback, useEffect, useMemo, useState } from "react";
import { X, LayoutList, Locate } from "lucide-react";
import { useStructural } from "../lib/store";
import { loadComponentsByType, type TypeScanRow } from "./componentsByType";
import { RenderWidget, registerWidget, type WidgetProps } from "./registry";
import type { Widget } from "../lib/ui/types";

const lastSeg = (t: string) => t.toLowerCase().split(/[:/.\\]+/).filter(Boolean).pop() ?? t.toLowerCase();
const typeMatches = (ct: string | undefined, kind: string) =>
  !!ct && (ct.toLowerCase() === kind.toLowerCase() || lastSeg(ct) === kind.toLowerCase());

interface Row { uid: number; name: string; path?: string; elsewhere?: boolean }

function TabbedComponents({ node, ctx }: WidgetProps) {
  const inner = node.inner as Widget | undefined;
  const kind = inner?.type ?? "";                    // last-segment match (in-folder)
  const fullType = (node.fullType as string) ?? "";  // exact type (global scan)
  const indexLabel = (node.indexLabel as string) ?? "components";

  // In-folder matches (live from the structural store).
  const components = useStructural((s) => s.components);
  const inFolder = useMemo(
    () => [...components.values()].filter((c) => typeMatches(c.type, kind)).map((c) => ({ uid: c.uid, name: c.name || `#${c.uid}`, path: c.path })),
    [components, kind],
  );
  // Global scan of the exact type (poll), merged in as "elsewhere".
  const [global, setGlobal] = useState<TypeScanRow[]>([]);
  useEffect(() => {
    if (!fullType) { setGlobal([]); return; }
    let alive = true;
    const tick = () => loadComponentsByType(fullType).then((r) => { if (alive) setGlobal(r); }).catch(() => {});
    tick();
    const id = setInterval(tick, 4000);
    return () => { alive = false; clearInterval(id); };
  }, [fullType]);

  const rows: Row[] = useMemo(() => {
    const seen = new Set(inFolder.map((r) => r.uid));
    const local: Row[] = inFolder.map((r) => ({ uid: r.uid, name: r.name, path: r.path }));
    const elsewhere: Row[] = global
      .filter((g) => !seen.has(g.uid))
      .map((g) => ({ uid: g.uid, name: g.name ?? `#${g.uid}`, path: g.path, elsewhere: true }));
    return [...local, ...elsewhere].sort((a, b) => a.name.localeCompare(b.name));
  }, [inFolder, global]);

  const [openUids, setOpenUids] = useState<number[]>([]);
  const [active, setActive] = useState<number | "index">("index");
  const openTab = useCallback((uid: number) => { setOpenUids((p) => (p.includes(uid) ? p : [...p, uid])); setActive(uid); }, []);
  // Right-click "Open UX" on a node → open that component's tab here.
  useEffect(() => {
    if (ctx.focusUid != null) openTab(ctx.focusUid);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ctx.focusNonce]);
  const closeTab = useCallback((uid: number) => {
    setOpenUids((p) => {
      const i = p.indexOf(uid);
      const next = p.filter((u) => u !== uid);
      setActive((a) => (a !== uid ? a : next[i] ?? next[i - 1] ?? "index"));
      return next;
    });
  }, []);
  const nameOf = (uid: number) => rows.find((r) => r.uid === uid)?.name ?? `#${uid}`;

  if (!inner) return <div style={{ padding: 12, color: "hsl(var(--crit))", fontSize: 12 }}>tabbedEditor: missing `inner` widget</div>;

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, color: "hsl(var(--foreground))" }}>
      <div style={{ display: "flex", gap: 1, background: "hsl(var(--background))", borderBottom: "1px solid hsl(var(--border))", flexShrink: 0, overflowX: "auto" }}>
        <Tab active={active === "index"} onClick={() => setActive("index")} pinned><LayoutList size={13} /> All</Tab>
        {openUids.map((uid) => (
          <Tab key={uid} active={active === uid} onClick={() => setActive(uid)}>
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 160 }}>{nameOf(uid)}</span>
            {ctx.locate && (
              <span role="button" title="Locate on canvas" onClick={(e) => { e.stopPropagation(); ctx.locate!(uid); }} style={{ display: "inline-flex", marginLeft: 2, color: "hsl(var(--muted-foreground))" }}>
                <Locate size={12} />
              </span>
            )}
            <span role="button" title="Close" onClick={(e) => { e.stopPropagation(); closeTab(uid); }} style={{ display: "inline-flex", marginLeft: 2, color: "hsl(var(--muted-foreground))" }}>
              <X size={12} />
            </span>
          </Tab>
        ))}
      </div>
      <div style={{ flex: 1, minHeight: 0, position: "relative" }}>
        <div style={{ position: "absolute", inset: 0, display: active === "index" ? "block" : "none", overflow: "auto" }}>
          <Index rows={rows} openUids={openUids} active={active} onOpen={openTab} label={indexLabel} />
        </div>
        {openUids.map((uid) => (
          <div key={uid} style={{ position: "absolute", inset: 0, display: active === uid ? "block" : "none" }}>
            <RenderWidget node={inner} ctx={{ ...ctx, componentUid: uid, active: active === uid }} />
          </div>
        ))}
      </div>
    </div>
  );
}

function Tab({ active, onClick, pinned, children }: { active: boolean; onClick: () => void; pinned?: boolean; children: React.ReactNode }) {
  return (
    <button onClick={onClick} style={{
      display: "flex", alignItems: "center", gap: 5, padding: "6px 10px", fontSize: 12, cursor: "pointer",
      border: "none", borderRight: "1px solid hsl(var(--border))", borderBottom: active ? "2px solid hsl(var(--cool))" : "2px solid transparent",
      background: active ? "hsl(var(--secondary))" : "transparent", color: active ? "hsl(var(--foreground))" : "hsl(var(--muted-foreground))",
      whiteSpace: "nowrap", flexShrink: 0, fontWeight: pinned ? 600 : 400,
    }}>{children}</button>
  );
}

function Index({ rows, openUids, active, onOpen, label }: {
  rows: Row[]; openUids: number[]; active: number | "index"; onOpen: (uid: number) => void; label: string;
}) {
  return (
    <div style={{ padding: 8, display: "flex", flexDirection: "column", gap: 4 }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 6, padding: "2px 4px 6px" }}>
        <span style={{ fontSize: 12, fontWeight: 600 }}>{label}</span>
        <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 11 }}>({rows.length})</span>
      </div>
      {rows.length === 0 ? (
        <div style={{ color: "hsl(var(--muted-foreground))", fontSize: 12, padding: 6 }}>none</div>
      ) : (
        rows.map((r) => (
          <button key={r.uid} onClick={() => onOpen(r.uid)} style={{
            display: "flex", alignItems: "center", gap: 8, padding: "7px 9px", textAlign: "left",
            background: active === r.uid ? "hsl(var(--secondary))" : "hsl(var(--card))", border: "1px solid hsl(var(--border))", borderRadius: 5,
            cursor: "pointer", color: "hsl(var(--foreground))", fontSize: 12,
          }}>
            <span style={{ fontWeight: 500, flexShrink: 0 }}>{r.name}</span>
            {r.path && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{r.path}</span>}
            <span style={{ flex: 1 }} />
            {openUids.includes(r.uid) && <span style={{ color: "hsl(var(--muted-foreground))", fontSize: 10 }}>open</span>}
          </button>
        ))
      )}
    </div>
  );
}

registerWidget("tabbedEditor", TabbedComponents);

export default TabbedComponents;
