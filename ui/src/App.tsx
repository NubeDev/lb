// The app shell — a real collaboration app over a real session (collaboration scope). The hardcoded
// S2 demo identity (`WS`/`CHANNEL`/`AUTHOR`) is gone: identity now comes from `useSession` (a verified
// token). Logged out → the login screen; logged in → the nav rail + workspace/channel switchers + the
// selected surface. Layout + wiring only; each surface owns its data (FILE-LAYOUT).

import { useState } from "react";

import { useSession, CAP, hasCap, isAdmin } from "@/lib/session";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { LoginView } from "./features/session";
import { NavRail, type Surface, type CoreSurface } from "./features/shell";
import { WorkspaceSwitcher } from "./features/workspace";
import { ChannelList, ChannelView } from "./features/channel";
import { MembersView } from "./features/members";
import { InboxView } from "./features/inbox";
import { OutboxView } from "./features/outbox";
import { AdminView } from "./features/admin";
import { ExtensionsView } from "./features/extensions";
import { DataView } from "./features/data";
import { SystemView } from "./features/system";
import { IngestView } from "./features/ingest";
import { DashboardView } from "./features/dashboard";
import { ExtHost, useExtensionPages } from "./features/ext-host";

function sidebarDefaultOpen() {
  if (typeof document === "undefined") return true;
  return !document.cookie.split("; ").includes("sidebar_state=false");
}

export function App() {
  const { session, signIn, signOut } = useSession();
  const [surface, setSurface] = useState<Surface>("channels");
  const [channel, setChannel] = useState("general");

  // Extension PAGES (ui-federation scope): installed extensions that declare a `[ui]` block contribute
  // a cap-gated sidebar slot. Discovered from `ext.list` (only visible to a session that can list
  // extensions — the gateway re-checks the page's bridged calls regardless). Called unconditionally
  // (before the logged-out early return) so the hook order is stable; the empty `ws` disables it.
  const extPages = useExtensionPages(
    session && hasCap(session.caps, CAP.extList) ? session.workspace : "",
  );

  if (!session) {
    return <LoginView onSignIn={signIn} />;
  }

  const { workspace, principal, caps } = session;
  // Switching workspace is a re-login (the workspace is the token's hard wall §7), keeping identity.
  const switchWorkspace = (ws: string) => void signIn(principal, ws);

  // Cap-gate the admin surfaces' VISIBILITY (admin-console scope). This is convenience only — the
  // gateway re-checks every verb server-side, so a forged call by a non-admin is denied regardless
  // (proven in role/gateway/tests/admin_routes_test.rs). Hiding the controls just avoids dead buttons.
  const allowed: CoreSurface[] = ["channels", "members", "inbox", "outbox"];
  // dashboard: the Dashboards page shows for any session that may list dashboards (member-level —
  // gate 3 / ownership still decides which specific dashboards they see). Gateway re-checks each verb.
  if (hasCap(caps, CAP.dashboardList)) allowed.push("dashboards");
  // data-console: the Ingest page shows for any session that may list series (member-level); the Data
  // page (the admin DB browser) shows only for a session holding `store.scan` — it relaxes gate 3, so
  // it is admin-only. The gateway re-checks every verb server-side regardless.
  if (hasCap(caps, CAP.seriesList)) allowed.push("ingest");
  if (hasCap(caps, CAP.storeScan)) allowed.push("data");
  // system-map: the System page (the topology + status console) reads across every subsystem, so it
  // is admin-only — shown for a session holding `system.overview`. The gateway re-checks regardless.
  if (hasCap(caps, CAP.systemOverview)) allowed.push("system");
  if (isAdmin(caps)) allowed.push("admin");
  if (hasCap(caps, CAP.extList)) allowed.push("extensions");

  // The active surface may be a core one or an extension page (`ext:<id>`). Fall back to channels if
  // the selected core surface isn't allowed or the selected extension page no longer exists.
  const activeExt = surface.startsWith("ext:") ? surface.slice(4) : null;
  const activeExtPage = activeExt ? extPages.find((p) => p.ext === activeExt) : undefined;
  const active: Surface = activeExtPage
    ? surface
    : allowed.includes(surface as CoreSurface)
      ? surface
      : "channels";

  return (
    <SidebarProvider defaultOpen={sidebarDefaultOpen()} className="h-full bg-bg">
      <NavRail
        active={active}
        onSelect={setSurface}
        onSignOut={signOut}
        allowed={allowed}
        extSlots={extPages.map((p) => ({ ext: p.ext, label: p.ui.label }))}
      />

      <SidebarInset className="min-w-0 overflow-hidden">
        <div className="flex h-full min-w-0 overflow-hidden">
          {active === "channels" && (
            <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-panel shadow-sm shadow-black/5">
              <WorkspaceSwitcher current={workspace} onSwitch={switchWorkspace} />
              <ChannelList ws={workspace} selected={channel} onSelect={setChannel} />
            </aside>
          )}

          <div className="min-w-0 flex-1 overflow-hidden">
            {active === "channels" && (
              <ChannelView ws={workspace} channel={channel} author={principal} />
            )}
            {active === "members" && <MembersView ws={workspace} />}
            {active === "dashboards" && <DashboardView ws={workspace} />}
            {active === "ingest" && <IngestView ws={workspace} />}
            {active === "data" && <DataView ws={workspace} />}
            {active === "system" && (
              <SystemView ws={workspace} onNavigate={setSurface} allowedSurfaces={allowed} />
            )}
            {active === "inbox" && <InboxView ws={workspace} />}
            {active === "outbox" && <OutboxView ws={workspace} />}
            {active === "admin" && <AdminView ws={workspace} caps={caps} />}
            {active === "extensions" && <ExtensionsView ws={workspace} />}
            {activeExtPage && (
              <ExtHost ext={activeExtPage.ext} ui={activeExtPage.ui} workspace={workspace} />
            )}
          </div>
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}
