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
import { ChannelView } from "@/features/channel";
import { DashboardView } from "@/features/dashboard";
import { PanelPage } from "@/features/panel";
import { DataStudioView } from "@/features/data-studio";
import { DataView } from "@/features/data";
import { DatasourcesAdmin, DatasourceDetailPage } from "@/features/datasources";
import { ExtHost, ExtErrorBoundary } from "@/features/ext-host";
import { ExtensionsView } from "@/features/extensions";
import { StudioShell, type StudioTab } from "@/features/studio";
import { FlowsView } from "@/features/flows";
import { TelemetryView } from "@/features/telemetry";
import { InboxView } from "@/features/inbox";
import { IngestView } from "@/features/ingest";
import { RulesView } from "@/features/rules";
import { RemindersView } from "@/features/reminders";
import { OutboxView } from "@/features/outbox";
import { type CoreSurface } from "@/features/shell";
import { SettingsView } from "@/features/settings";
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
import { CAP, hasCap } from "@/lib/session";

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

// The standalone library-panel page (library-panels scope): a directly-linkable single panel, no
// dashboard. Reuses the dashboard search grammar (range + `?var-`); cap-gated on `panel.get` in the
// component (it isn't a nav CoreSurface — a deep link/nav entry points here, but it's not in the rail).
const panelRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/panel/$id",
  validateSearch: validateDashboardSearch,
  component: PanelRoute,
});

// Data Studio (data-studio scope): explore any source + build a reusable panel. Reuses the dashboard
// search grammar (range + `?var-`); cap-gated on `series.list` in the component (member-level explore),
// the save-as-library action gated on `panel.save` inside the view.
const dataStudioRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/data-studio",
  validateSearch: validateDashboardSearch,
  component: DataStudioRoute,
});

const datasourceDetailRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/datasources/$name",
  component: DatasourceDetailRoute,
});

// Settings tabs are deep-linkable: `/settings/<tab>` (preferences | agent | theme). The bare
// `/settings` route redirects to the default tab, so an old link keeps working.
const settingsTabRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/settings/$tab",
  component: SettingsTabRoute,
});

const flowDetailRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/flows/$id",
  component: FlowDetailRoute,
});

// `/rules/$rule` opens that saved rule; bare `/rules` is a fresh buffer. Same view, URL reflects the
// open rule so deep links + back/forward work (mirrors the flows detail route). Cap-gated below since
// this detail route isn't wrapped by the bare `/rules` CoreGate.
const rulesDetailRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/rules/$rule",
  component: RulesDetailRoute,
});

const extRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/ext/$id",
  component: ExtRoute,
});

// Studio section: one page, two route-driven tabs. `extensions` (manage installed) and `build` (the
// scaffold wizard) are distinct CoreSurfaces with their OWN caps — each tab renders behind its own
// CoreGate, so a session with only one cap deep-links straight to the tab it can reach. Bare `/studio`
// (and the legacy `/extensions`) redirect to the first tab the session is allowed.
const studioExtensionsRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/studio/extensions",
  component: () => (
    <CoreGate surface="extensions">
      <StudioSection tab="extensions" />
    </CoreGate>
  ),
});

const studioBuildRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/studio/build",
  component: () => (
    <CoreGate surface="studio">
      <StudioSection tab="build" />
    </CoreGate>
  ),
});

const studioIndexRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/studio",
  component: StudioDefaultRedirect,
});

// Legacy `/extensions` deep links land on the Extensions tab (or the first allowed tab).
const extensionsRedirectRoute = createRoute({
  getParentRoute: () => tenantRoute,
  path: "/extensions",
  component: StudioDefaultRedirect,
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
    dashboardsRoute,
    panelRoute,
    dataStudioRoute,
    coreRoute("/rules", "rules", () => <Rules />),
    rulesDetailRoute,
    coreRoute("/flows", "flows", () => <Flows />),
    flowDetailRoute,
    coreRoute("/datasources", "datasources", () => <Datasources />),
    datasourceDetailRoute,
    coreRoute("/reminders", "reminders", () => <Reminders />),
    coreRoute("/ingest", "ingest", () => <Ingest />),
    coreRoute("/data", "data", () => <Data />),
    coreRoute("/system", "system", () => <System />),
    coreRoute("/system/mcp", "system-mcp", () => <McpService />),
    coreRoute("/system/acp", "system-acp", () => <AcpService />),
    coreRoute("/telemetry", "telemetry", () => <Telemetry />),
    coreRoute("/inbox", "inbox", () => <Inbox />),
    coreRoute("/outbox", "outbox", () => <Outbox />),
    coreRoute("/admin", "admin", () => <Admin />),
    studioExtensionsRoute,
    studioBuildRoute,
    studioIndexRoute,
    extensionsRedirectRoute,
    coreRoute("/settings", "settings", () => <SettingsPage />),
    settingsTabRoute,
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
  const navigate = useNavigate({ from: "/t/$ws/channels" });
  return (
    <ChannelView
      ws={ctx.workspace}
      channel={c}
      author={ctx.principal}
      onSelectChannel={(channel) => void navigate({ search: { c: channel } })}
      onSwitchWorkspace={ctx.switchWorkspace}
    />
  );
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

function DataStudioRoute() {
  const ctx = useAppRoutingContext();
  const range = dataStudioRoute.useSearch();
  const navigate = useNavigate({ from: "/t/$ws/data-studio" });
  if (!ctx.allowed.includes("data-studio")) return <DefaultRedirect />;
  return (
    <DataStudioView
      ws={ctx.workspace}
      range={range}
      onSearchChange={(next) => void navigate({ search: next })}
    />
  );
}

function PanelRoute() {
  const ctx = useAppRoutingContext();
  const { id } = panelRoute.useParams();
  const range = panelRoute.useSearch();
  const navigate = useNavigate({ from: "/t/$ws/panel/$id" });
  // Cap-gate the standalone page on `panel.get` (library-panels Decision: distinct read cap, no
  // piggybacking on dashboard.list). The gateway re-checks server-side regardless; this is the UI lens.
  if (!hasCap(ctx.caps, CAP.panelGet)) return <DefaultRedirect />;
  return (
    <PanelPage
      ws={ctx.workspace}
      id={decodeURIComponent(id)}
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
  // Wall off the extension: a render-time throw inside it stays inside it, not up through the shell
  // (nav + sidebar). Keyed on the ext id so switching extensions re-arms after a crash.
  return (
    <ExtErrorBoundary ext={page.ext} resetKey={page.ext}>
      <ExtHost ext={page.ext} ui={page.ui} workspace={ctx.workspace} />
    </ExtErrorBoundary>
  );
}

// Drives the rules surface with the open rule reflected in the URL: `/rules` (fresh buffer) and
// `/rules/$rule` (that rule open) render the same view; opening/creating/deleting navigates between
// them so deep links and back/forward work (mirrors FlowsSurface).
function RulesSurface({ ruleId }: { ruleId: string | null }) {
  const ctx = useAppRoutingContext();
  const navigate = useNavigate();
  return (
    <RulesView
      ws={ctx.workspace}
      ruleId={ruleId}
      onSelectRule={(id) =>
        void navigate({
          to: id
            ? `/t/${encodeURIComponent(ctx.workspace)}/rules/${encodeURIComponent(id)}`
            : `/t/${encodeURIComponent(ctx.workspace)}/rules`,
          replace: true,
        })
      }
    />
  );
}

function Rules() {
  return <RulesSurface ruleId={null} />;
}

function RulesDetailRoute() {
  const ctx = useAppRoutingContext();
  const { rule } = rulesDetailRoute.useParams();
  if (!ctx.allowed.includes("rules")) return <DefaultRedirect />;
  return <RulesSurface ruleId={decodeURIComponent(rule)} />;
}

// Drives the flows surface with the open flow reflected in the URL: `/flows` (none open) and
// `/flows/$id` (that flow open) render the same view; selecting/creating/deleting navigates between
// them so deep links and back/forward work. Cap gate is inherited from the parent (CoreGate wraps
// the bare `/flows`; the detail route re-checks below since it isn't wrapped).
function FlowsSurface({ flowId }: { flowId: string | null }) {
  const ctx = useAppRoutingContext();
  const navigate = useNavigate();
  return (
    <FlowsView
      ws={ctx.workspace}
      flowId={flowId}
      onSelectFlow={(id) =>
        void navigate({
          to: id
            ? `/t/${encodeURIComponent(ctx.workspace)}/flows/${encodeURIComponent(id)}`
            : `/t/${encodeURIComponent(ctx.workspace)}/flows`,
          replace: true,
        })
      }
    />
  );
}

function Flows() {
  return <FlowsSurface flowId={null} />;
}

function FlowDetailRoute() {
  const ctx = useAppRoutingContext();
  const { id } = flowDetailRoute.useParams();
  if (!ctx.allowed.includes("flows")) return <DefaultRedirect />;
  return <FlowsSurface flowId={decodeURIComponent(id)} />;
}

function Reminders() {
  return <RemindersView ws={useAppRoutingContext().workspace} />;
}

function Telemetry() {
  // The console reads the session (workspace + caps) directly; no prop needed (the ws wall is the
  // token, server-side).
  return <TelemetryView />;
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

// The Studio tabs a session may SEE, in display order, gated by their CoreSurface caps (extensions →
// ext.list, build → devkit.templates). Drives the tab bar and the default-redirect target.
function allowedStudioTabs(allowed: readonly CoreSurface[]): StudioTab[] {
  const tabs: StudioTab[] = [];
  if (allowed.includes("extensions")) tabs.push("extensions");
  if (allowed.includes("studio")) tabs.push("build");
  return tabs;
}

// One merged page; the active tab comes from the route and navigating a tab changes the URL, so each
// tab is deep-linkable and back/forward works.
function StudioSection({ tab }: { tab: StudioTab }) {
  const ctx = useAppRoutingContext();
  const navigate = useNavigate();
  const allowedTabs = allowedStudioTabs(ctx.allowed);
  return (
    <StudioShell
      ws={ctx.workspace}
      tab={tab}
      allowedTabs={allowedTabs}
      onSelectTab={(t) =>
        void navigate({
          to: `/t/${encodeURIComponent(ctx.workspace)}/studio/${t === "build" ? "build" : "extensions"}`,
        })
      }
    >
      {tab === "extensions" ? (
        <ExtensionsView ws={ctx.workspace} embedded />
      ) : (
        <StudioView ws={ctx.workspace} embedded />
      )}
    </StudioShell>
  );
}

// Bare `/studio` (and legacy `/extensions`): jump to the first tab the session is allowed; if neither,
// fall through to the workspace default (the cap-deny path).
function StudioDefaultRedirect() {
  const ctx = useAppRoutingContext();
  const location = useLocation();
  const navigate = useNavigate();
  const first = allowedStudioTabs(ctx.allowed)[0];
  const href = first
    ? `/t/${encodeURIComponent(ctx.workspace)}/studio/${first === "build" ? "build" : "extensions"}`
    : null;
  // Fire once via effect: a render-time <Navigate> to a dynamic href re-evaluates every commit and
  // trips React's update cap (same pattern as TenantlessRedirect).
  useEffect(() => {
    if (href) void navigate({ to: href, replace: true });
  }, [href, navigate, location.pathname]);
  if (!first) return <DefaultRedirect />;
  return null;
}

function SettingsPage() {
  // Bare `/settings` redirects to the default tab so every Settings surface has a canonical deep link.
  const ctx = useAppRoutingContext();
  return (
    <Navigate to="/t/$ws/settings/$tab" params={{ ws: ctx.workspace, tab: "preferences" }} replace />
  );
}

function SettingsTabRoute() {
  const ctx = useAppRoutingContext();
  const { tab } = settingsTabRoute.useParams();
  const navigate = useNavigate();
  return (
    <CoreGate surface="settings">
      <SettingsView
        ws={ctx.workspace}
        caps={ctx.caps}
        tab={tab}
        onTabChange={(next) =>
          void navigate({ to: "/t/$ws/settings/$tab", params: { ws: ctx.workspace, tab: next }, replace: true })
        }
      />
    </CoreGate>
  );
}
