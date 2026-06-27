// The app shell — a real collaboration app over a real session (collaboration scope). The hardcoded
// S2 demo identity (`WS`/`CHANNEL`/`AUTHOR`) is gone: identity now comes from `useSession` (a verified
// token). Logged out → the login screen; logged in → the nav rail + workspace/channel switchers + the
// selected surface. Layout + wiring only; each surface owns its data (FILE-LAYOUT).

import { useState } from "react";

import { useSession, CAP, hasCap, isAdmin } from "@/lib/session";
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
import { IngestView } from "./features/ingest";
import { ExtHost, useExtensionPages } from "./features/ext-host";

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
  // data-console: the Ingest page shows for any session that may list series (member-level); the Data
  // page (the admin DB browser) shows only for a session holding `store.scan` — it relaxes gate 3, so
  // it is admin-only. The gateway re-checks every verb server-side regardless.
  if (hasCap(caps, CAP.seriesList)) allowed.push("ingest");
  if (hasCap(caps, CAP.storeScan)) allowed.push("data");
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
    <div className="flex h-full">
      <NavRail
        active={active}
        onSelect={setSurface}
        onSignOut={signOut}
        allowed={allowed}
        extSlots={extPages.map((p) => ({ ext: p.ext, label: p.ui.label }))}
      />

      {active === "channels" && (
        <aside className="flex w-56 flex-col border-r border-border bg-panel">
          <WorkspaceSwitcher current={workspace} onSwitch={switchWorkspace} />
          <ChannelList ws={workspace} selected={channel} onSelect={setChannel} />
        </aside>
      )}

      <main className="flex-1">
        {active === "channels" && (
          <ChannelView ws={workspace} channel={channel} author={principal} />
        )}
        {active === "members" && <MembersView ws={workspace} />}
        {active === "ingest" && <IngestView ws={workspace} />}
        {active === "data" && <DataView ws={workspace} />}
        {active === "inbox" && <InboxView ws={workspace} />}
        {active === "outbox" && <OutboxView ws={workspace} />}
        {active === "admin" && <AdminView ws={workspace} caps={caps} />}
        {active === "extensions" && <ExtensionsView ws={workspace} />}
        {activeExtPage && (
          <ExtHost ext={activeExtPage.ext} ui={activeExtPage.ui} workspace={workspace} />
        )}
      </main>
    </div>
  );
}
