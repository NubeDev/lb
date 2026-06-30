// The authenticated shell around routed pages. The shadcn shell stays the same; TanStack Router owns
// only which surface renders inside SidebarInset and which URL NavRail navigates to.

import { Outlet, useLocation, useNavigate } from "@tanstack/react-router";

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { ChannelList } from "@/features/channel";
import { NavRail, type Surface } from "@/features/shell";
import { WorkspaceSwitcher } from "@/features/workspace";
import { useAppRoutingContext } from "./RoutingContextProvider";
import { DEFAULT_CHANNEL } from "./search";
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
  const search = location.search as Record<string, unknown>;
  const channel =
    active === "channels" && typeof search.c === "string"
      ? (search.c ?? DEFAULT_CHANNEL)
      : DEFAULT_CHANNEL;

  const selectSurface = (surface: Surface) => {
    void navigate({
      to: fullPathForSurface(ctx.workspace, surface),
      search: surface === "channels" ? { c: DEFAULT_CHANNEL } : undefined,
    });
  };

  const selectChannel = (channel: string) => {
    void navigate({
      to: fullPathForSurface(ctx.workspace, "channels"),
      search: { c: channel },
    });
  };

  return (
    <SidebarProvider defaultOpen={sidebarDefaultOpen()} className="h-full bg-bg">
      <NavRail
        active={active}
        onSelect={selectSurface}
        onSignOut={ctx.onSignOut}
        allowed={ctx.allowed}
        extSlots={ctx.extPages.map((p) => ({ ext: p.ext, label: p.ui.label }))}
      />

      <SidebarInset className="min-w-0 overflow-hidden">
        <div className="flex h-full min-w-0 overflow-hidden">
          {active === "channels" && (
            <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-panel shadow-sm shadow-black/5">
              <WorkspaceSwitcher
                current={ctx.workspace}
                principal={ctx.principal}
                onSwitch={ctx.switchWorkspace}
              />
              <ChannelList ws={ctx.workspace} selected={channel} onSelect={selectChannel} />
            </aside>
          )}

          <div className="min-w-0 flex-1 overflow-hidden">
            <Outlet />
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
