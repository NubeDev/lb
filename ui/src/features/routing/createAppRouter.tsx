// TanStack Router tree for the shell URL grammar. Hash history is used by createAppRouter(), so the
// same fragment URLs work under browser origins and Tauri custom-protocol origins.

import { useEffect } from "react";
import {
  Navigate,
  createHashHistory,
  createRootRouteWithContext,
  createRoute,
  createRouter,
  redirect,
  useLocation,
  useNavigate,
} from "@tanstack/react-router";

import { AdminView } from "@/features/admin";
import { ChainsView } from "@/features/chains";
import { ChannelView } from "@/features/channel";
import { DashboardView } from "@/features/dashboard";
import { DataView } from "@/features/data";
import { DatasourcesAdmin, DatasourceDetailPage } from "@/features/datasources";
import { ExtHost } from "@/features/ext-host";
import { ExtensionsView } from "@/features/extensions";
import { InboxView } from "@/features/inbox";
import { IngestView } from "@/features/ingest";
import { MembersView } from "@/features/members";
import { RulesView } from "@/features/rules";
import { RemindersView } from "@/features/reminders";
import { OutboxView } from "@/features/outbox";
import { type CoreSurface } from "@/features/shell";
import { StudioView } from "@/features/studio";
import { SystemView } from "@/features/system";
import { AcpServiceView } from "@/features/system-acp";
import { McpServiceView } from "@/features/system-mcp";
import { RoutedShell } from "./RoutedShell";
import { useAppRoutingContext } from "./RoutingContextProvider";
import type { RoutingContext } from "./context";
import {
  DEFAULT_CHANNEL,
  validateChannelSearch,
  validateDashboardSearch,
} from "./search";
import { fullPathForSurface, pathForSurface, surfaceForPath, tenantPath } from "./surface";

const rootRoute = createRootRouteWithContext<RoutingContext>()({
  component: RootRoute,
  notFoundComponent: TenantlessRedirect,
});

// Bare `/` (and any path that lost its tenant prefix) → redirect to the token's workspace. The
// workspace is NEVER read from here; it comes from the verified session in the router context.
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: TenantlessRedirect,
});

// The tenant layout: every surface lives under `/t/<ws>`. `beforeLoad` is the security-relevant
// guard — the `$ws` segment is a deep-link hint, NOT an authorization input. If a pasted link's
// workspace differs from the recipient's verified token, we rewrite the URL to the token's
// workspace (never fetch the URL's). The gateway re-derives the real workspace from the token
// regardless (§7), so this guard is convenience + honest URLs, not the wall itself.
const tenantRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/t/$ws",
  beforeLoad: ({ params, context, location }) => {
    if (params.ws !== context.workspace) {
      // Keep the surface the link pointed at; only the workspace segment is rewritten to the
      // recipient's verified workspace. The URL's `$ws` is never trusted as data.
      const surface = surfaceForPath(location.pathname);
      throw redirect({
        href: `${tenantPath(context.workspace, pathForSurface(surface))}${location.searchStr}`,
        replace: true,
      });
    }
  },
});

const tenantIndexRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/",
  component: DefaultRedirect,
});

const channelsRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/channels",
  validateSearch: validateChannelSearch,
  component: ChannelsRoute,
});

const dashboardsRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/dashboards",
  validateSearch: validateDashboardSearch,
  component: DashboardsRoute,
});

const datasourceDetailRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/datasources/$name",
  component: DatasourceDetailRoute,
});

const extRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/ext/$id",
  component: ExtRoute,
});

function coreRoute(path: string, surface: CoreSurface, component: () => JSX.Element) {
  return createRoute({
    getParentRoute: () => tenantRoute,
    path,
    component: () => <CoreGate surface={surface}>{component()}</CoreGate>,
  });
}

const routeTree = rootRoute.addChildren([
  indexRoute,
  tenantRoute.addChildren([
    tenantIndexRoute,
    channelsRoute,
    coreRoute("/members", "members", () => <Members />),
    dashboardsRoute,
    coreRoute("/rules", "rules", () => <Rules />),
    coreRoute("/chains", "chains", () => <Chains />),
    coreRoute("/datasources", "datasources", () => <Datasources />),
    datasourceDetailRoute,
    coreRoute("/reminders", "reminders", () => <Reminders />),
    coreRoute("/ingest", "ingest", () => <Ingest />),
    coreRoute("/data", "data", () => <Data />),
    coreRoute("/system", "system", () => <System />),
    coreRoute("/system/mcp", "system-mcp", () => <McpService />),
    coreRoute("/system/acp", "system-acp", () => <AcpService />),
    coreRoute("/inbox", "inbox", () => <Inbox />),
    coreRoute("/outbox", "outbox", () => <Outbox />),
    coreRoute("/admin", "admin", () => <Admin />),
    coreRoute("/extensions", "extensions", () => <Extensions />),
    coreRoute("/studio", "studio", () => <Studio />),
    extRoute,
  ]),
]);

export function createAppRouter() {
  return createRouter({
    routeTree,
    history: createHashHistory(),
    context: undefined as unknown as RoutingContext,
  });
}

export type AppRouter = ReturnType<typeof createAppRouter>;

declare module "@tanstack/react-router" {
  interface Register {
    router: AppRouter;
  }
}

function RootRoute() {
  return <RoutedShell />;
}

// Cap-deny fallback: force the default surface within the token's workspace.
function DefaultRedirect() {
  const ctx = useAppRoutingContext();
  return (
    <Navigate
      to="/t/$ws/channels"
      params={{ ws: ctx.workspace }}
      search={{ c: DEFAULT_CHANNEL }}
      replace
    />
  );
}

// Index + not-found fallback for tenant-LESS paths (e.g. a `#/dashboards` link minted before the
// `/t/<ws>` grammar, or `/`). Preserve the intended surface and graft on the token's workspace —
// the workspace is taken from the verified session, never from the URL. Fired once via effect (a
// render-time `<Navigate>` to a dynamic href re-evaluates every commit and trips React's update cap).
function TenantlessRedirect() {
  const ctx = useAppRoutingContext();
  const location = useLocation();
  const navigate = useNavigate();
  const surface = surfaceForPath(location.pathname);
  const href = `${tenantPath(ctx.workspace, pathForSurface(surface))}${location.searchStr}`;
  useEffect(() => {
    void navigate({ to: href, replace: true });
  }, [href, navigate]);
  return null;
}

function CoreGate({ surface, children }: { surface: CoreSurface; children: JSX.Element }) {
  const ctx = useAppRoutingContext();
  if (!ctx.allowed.includes(surface)) return <DefaultRedirect />;
  return children;
}

function ChannelsRoute() {
  const ctx = useAppRoutingContext();
  const { c } = channelsRoute.useSearch();
  return <ChannelView ws={ctx.workspace} channel={c} author={ctx.principal} />;
}

function DashboardsRoute() {
  const ctx = useAppRoutingContext();
  const range = dashboardsRoute.useSearch();
  const navigate = useNavigate({ from: "/t/$ws/dashboards" });
  if (!ctx.allowed.includes("dashboards")) return <DefaultRedirect />;
  return (
    <DashboardView
      ws={ctx.workspace}
      range={range}
      onSearchChange={(next) => void navigate({ search: next })}
    />
  );
}

function ExtRoute() {
  const ctx = useAppRoutingContext();
  const { id } = extRoute.useParams();
  const page = ctx.extPages.find((p: { ext: string }) => p.ext === id);
  if (!page && ctx.extPagesLoading) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-muted">
        Loading extension page…
      </div>
    );
  }
  if (!page) return <DefaultRedirect />;
  return <ExtHost ext={page.ext} ui={page.ui} workspace={ctx.workspace} />;
}

function Members() {
  return <MembersView ws={useAppRoutingContext().workspace} />;
}

function Rules() {
  return <RulesView ws={useAppRoutingContext().workspace} />;
}

function Chains() {
  return <ChainsView ws={useAppRoutingContext().workspace} />;
}

function Reminders() {
  return <RemindersView ws={useAppRoutingContext().workspace} />;
}

function Datasources() {
  const ctx = useAppRoutingContext();
  const navigate = useNavigate();
  return (
    <DatasourcesAdmin
      ws={ctx.workspace}
      onOpen={(name) =>
        void navigate({
          to: `/t/${encodeURIComponent(ctx.workspace)}/datasources/${encodeURIComponent(name)}`,
        })
      }
    />
  );
}

function DatasourceDetailRoute() {
  const ctx = useAppRoutingContext();
  const { name } = datasourceDetailRoute.useParams();
  if (!ctx.allowed.includes("datasources")) return <DefaultRedirect />;
  return (
    <DatasourceDetailPage
      ws={ctx.workspace}
      name={decodeURIComponent(name)}
    />
  );
}

function Ingest() {
  return <IngestView ws={useAppRoutingContext().workspace} />;
}

function Data() {
  return <DataView ws={useAppRoutingContext().workspace} />;
}

function System() {
  const ctx = useAppRoutingContext();
  const navigate = useNavigate();
  return (
    <SystemView
      ws={ctx.workspace}
      allowedSurfaces={ctx.allowed}
      onNavigate={(surface) =>
        void navigate({ to: fullPathForSurface(ctx.workspace, surface) })
      }
    />
  );
}

function McpService() {
  const ctx = useAppRoutingContext();
  const navigate = useNavigate();
  return (
    <McpServiceView
      ws={ctx.workspace}
      onBack={() => void navigate({ to: fullPathForSurface(ctx.workspace, "system") })}
    />
  );
}

function AcpService() {
  const ctx = useAppRoutingContext();
  const navigate = useNavigate();
  return (
    <AcpServiceView
      ws={ctx.workspace}
      onBack={() => void navigate({ to: fullPathForSurface(ctx.workspace, "system") })}
    />
  );
}

function Inbox() {
  return <InboxView ws={useAppRoutingContext().workspace} />;
}

function Outbox() {
  return <OutboxView ws={useAppRoutingContext().workspace} />;
}

function Admin() {
  const ctx = useAppRoutingContext();
  return <AdminView ws={ctx.workspace} caps={ctx.caps} />;
}

function Extensions() {
  return <ExtensionsView ws={useAppRoutingContext().workspace} />;
}

function Studio() {
  return <StudioView ws={useAppRoutingContext().workspace} />;
}
