// The authenticated shell around routed pages. The shadcn shell stays the same; TanStack Router owns
// only which surface renders inside SidebarInset and which URL NavRail navigates to.

import { useRef, useState } from "react";
import { Outlet, useLocation, useNavigate } from "@tanstack/react-router";

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { NavRail, StatusBar, useResolvedNav, type Surface } from "@/features/shell";
import {
  AgentDock,
  DockLauncher,
  PageContextProvider,
  useDockChrome,
  useDockHotkey,
} from "@/features/agent-dock";
import { useAppRoutingContext } from "./RoutingContextProvider";
import { DEFAULT_CHANNEL, defaultDashboardSearch, type DashboardSearch } from "./search";
import { fullPathForSurface, surfaceForPath } from "./surface";

function sidebarDefaultOpen() {
  if (typeof document === "undefined") return true;
  return !document.cookie.split("; ").includes("sidebar_state=false");
}

export function RoutedShell() {
  const ctx = useAppRoutingContext();
  const navigate = useNavigate();
  const location = useLocation();
  const active = surfaceForPath(location.pathname);
  // The caller's resolved nav menu (nav scope) — NavRail renders it (already cap-stripped), falling
  // back to the built-in SURFACES when null (no nav / denied). Route gates are untouched.
  const resolvedItems = useResolvedNav(ctx.workspace);

  // The agent dock (agent-dock scope): shell-mounted so it outlives navigation. Chrome (open/width) is
  // owned here and shared with the StatusBar launcher; `mod+j` toggles it via one shell listener.
  const dock = useDockChrome();
  const [dockRunning, setDockRunning] = useState(false);
  const launcherRef = useRef<HTMLButtonElement>(null);
  useDockHotkey(dock.toggle);
  // Return focus to the launcher when the dock closes (scope decision 1: Escape returns focus).
  const closeDock = () => {
    dock.close();
    launcherRef.current?.focus();
  };

  const selectSurface = (surface: Surface) => {
    // The merged Studio rail entry (keyed `extensions`) routes through the bare `/studio` redirect so
    // the session lands on the first tab its caps allow (a build-only user gets Build, not a denied
    // Extensions tab). Every other surface goes straight to its own path.
    if (surface === "extensions") {
      void navigate({ to: `/t/${encodeURIComponent(ctx.workspace)}/studio` });
      return;
    }
    void navigate({
      to: fullPathForSurface(ctx.workspace, surface),
      search: surface === "channels" ? { c: DEFAULT_CHANNEL } : undefined,
    });
  };

  // reusable-pages: navigate to a specific board, applying a pinned/template binding as `?var-<name>=…`
  // (the nav link SETS the URL; after that the URL is the single source of truth — the shipped model).
  const selectDashboard = (dashboard: string, vars?: Record<string, string>) => {
    const id = dashboard.replace(/^dashboard:/, "");
    // Seed the default range (from/to) so the board opens with a valid window; the binding sets `?var-`.
    const search: DashboardSearch = { ...defaultDashboardSearch(), d: id };
    for (const [name, value] of Object.entries(vars ?? {})) search[`var-${name}`] = value;
    void navigate({ to: fullPathForSurface(ctx.workspace, "dashboards"), search });
  };

  return (
    <SidebarProvider defaultOpen={sidebarDefaultOpen()} className="h-full bg-bg">
      <NavRail
        active={active}
        onSelect={selectSurface}
        onSignOut={ctx.onSignOut}
        allowed={ctx.allowed}
        extSlots={ctx.extPages.map((p) => ({ ext: p.ext, label: p.ui.label }))}
        resolvedItems={resolvedItems}
        onSelectDashboard={selectDashboard}
      />

      <SidebarInset className="min-w-0 overflow-hidden">
        <PageContextProvider>
          <div className="flex h-full min-w-0 flex-col overflow-hidden">
            {/* The routed page + the agent dock share a horizontal row: the dock shrinks the page
                (reflow), and because it is shell-mounted it survives navigation (agent-dock scope). */}
            <div className="flex min-h-0 min-w-0 flex-1 overflow-hidden">
              <div className="min-h-0 min-w-0 flex-1 overflow-hidden">
                <Outlet />
              </div>
              {dock.open && (
                <AgentDock
                  ws={ctx.workspace}
                  principal={ctx.principal}
                  width={dock.width}
                  onWidth={dock.setWidth}
                  onClose={closeDock}
                  onRunningChange={setDockRunning}
                />
              )}
            </div>
            {/* The ops strip: session facts (workspace wall, identity, cap count) always in view; the
                dock launcher + run pip ride at its right edge. */}
            <StatusBar
              workspace={ctx.workspace}
              principal={ctx.principal}
              capCount={ctx.caps?.length ?? 0}
              trailing={
                <DockLauncher
                  ref={launcherRef}
                  open={dock.open}
                  running={dockRunning}
                  onToggle={dock.toggle}
                />
              }
            />
          </div>
        </PageContextProvider>
      </SidebarInset>
    </SidebarProvider>
  );
}
