// Generic IDE-style tab shell: a pinned index tab + closeable item tabs, with
// open/active state persisted per `persistKey` (so switching submenus keeps each
// one's tabs). Content is supplied via render props — Components/Scripts/Examples
// (and the scheduler) are thin configs on top of this.
//
// Open tabs stay mounted (display-toggled) so their inner state survives tab
// switches; `renderTab` receives `active` so only the visible one subscribes.

import { useCallback, useEffect, useState } from "react";
import { X } from "lucide-react";

interface ShellState { open: string[]; active: string }
const STORE = new Map<string, ShellState>(); // per-persistKey tab state (survives remounts)

export interface TabShellProps {
  /** stable key for persisting which tabs are open + active across remounts */
  persistKey: string;
  pinned: { id: string; label: string; icon?: React.ReactNode };
  /** render the index ("All") view; call `open(id)` to open an item tab */
  renderIndex: (open: (id: string) => void) => React.ReactNode;
  /** render an open tab's content */
  renderTab: (id: string, active: boolean) => React.ReactNode;
  tabLabel: (id: string) => string;
  /** extra inline controls in a tab (before the close button), e.g. a locate icon */
  tabExtra?: (id: string) => React.ReactNode;
  /** return false to block closing a tab (e.g. unsaved changes) */
  closeGuard?: (id: string) => boolean;
  /** one-shot request to open/focus a tab from outside (e.g. canvas right-click
   *  "Open UX"); `nonce` changes per request so the same id re-triggers. */
  openRequest?: { id: string; nonce?: number };
}

export function TabShell({ persistKey, pinned, renderIndex, renderTab, tabLabel, tabExtra, closeGuard, openRequest }: TabShellProps) {
  const [open, setOpen] = useState<string[]>(() => STORE.get(persistKey)?.open ?? []);
  const [active, setActive] = useState<string>(() => STORE.get(persistKey)?.active ?? pinned.id);
  useEffect(() => { STORE.set(persistKey, { open, active }); }, [persistKey, open, active]);

  const openTab = useCallback((id: string) => {
    setOpen((p) => (p.includes(id) ? p : [...p, id]));
    setActive(id);
  }, []);
  // External open request (canvas → "Open UX" → this component's tab).
  useEffect(() => {
    if (openRequest?.id) openTab(openRequest.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [openRequest?.nonce, openRequest?.id]);
  const closeTab = useCallback((id: string) => {
    if (closeGuard && !closeGuard(id)) return;
    setOpen((p) => {
      const i = p.indexOf(id);
      const next = p.filter((x) => x !== id);
      setActive((a) => (a !== id ? a : next[i] ?? next[i - 1] ?? pinned.id));
      return next;
    });
  }, [closeGuard, pinned.id]);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%", minHeight: 0, color: "hsl(var(--foreground))" }}>
      <div style={{ display: "flex", alignItems: "stretch", gap: 1, background: "hsl(var(--background))", borderBottom: "1px solid hsl(var(--border))", flexShrink: 0, overflowX: "auto" }}>
        <ShellTab active={active === pinned.id} onClick={() => setActive(pinned.id)} pinned>
          {pinned.icon}{pinned.label}
        </ShellTab>
        {open.map((id) => (
          <ShellTab key={id} active={active === id} onClick={() => setActive(id)}>
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", maxWidth: 160 }}>{tabLabel(id)}</span>
            {tabExtra?.(id)}
            <span role="button" title="Close" onClick={(e) => { e.stopPropagation(); closeTab(id); }} style={shellIconBtn}>
              <X size={12} />
            </span>
          </ShellTab>
        ))}
      </div>
      <div style={{ flex: 1, minHeight: 0, position: "relative" }}>
        <div style={{ position: "absolute", inset: 0, display: active === pinned.id ? "block" : "none", overflow: "auto" }}>
          {renderIndex(openTab)}
        </div>
        {open.map((id) => (
          <div key={id} style={{ position: "absolute", inset: 0, display: active === id ? "block" : "none" }}>
            {renderTab(id, active === id)}
          </div>
        ))}
      </div>
    </div>
  );
}

function ShellTab({ active, onClick, pinned, children }: { active: boolean; onClick: () => void; pinned?: boolean; children: React.ReactNode }) {
  return (
    <button
      onClick={onClick}
      style={{
        display: "flex", alignItems: "center", gap: 5, padding: "6px 10px", fontSize: 12, cursor: "pointer",
        border: "none", borderRight: "1px solid hsl(var(--border))", borderBottom: active ? "2px solid hsl(var(--cool))" : "2px solid transparent",
        background: active ? "hsl(var(--secondary))" : "transparent", color: active ? "hsl(var(--foreground))" : "hsl(var(--muted-foreground))",
        whiteSpace: "nowrap", flexShrink: 0, fontWeight: pinned ? 600 : 400,
      }}
    >
      {children}
    </button>
  );
}

export const shellIconBtn: React.CSSProperties = { display: "inline-flex", alignItems: "center", marginLeft: 2, padding: 1, borderRadius: 3, color: "hsl(var(--muted-foreground))" };
