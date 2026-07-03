// The authenticated shell around routed pages. The shadcn shell stays the same; TanStack Router owns
// only which surface renders inside SidebarInset and which URL NavRail navigates to.

import { Outlet, useLocation, useNavigate } from "@tanstack/react-router";

import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { NavRail, useResolvedNav, type Surface } from "@/features/shell";
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
  // The caller's resolved nav menu (nav scope) — NavRail renders it (already cap-stripped), falling
  // back to the built-in SURFACES when null (no nav / denied). Route gates are untouched.
  const resolvedItems = useResolvedNav(ctx.workspace);

  const selectSurface = (surface: Surface) => {
    void navigate({
      to: fullPathForSurface(ctx.workspace, surface),
      search: surface === "channels" ? { c: DEFAULT_CHANNEL } : undefined,
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
        resolvedItems={resolvedItems}
      />

      <SidebarInset className="min-w-0 overflow-hidden">
        <div className="h-full min-w-0 overflow-hidden">
          <Outlet />
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
