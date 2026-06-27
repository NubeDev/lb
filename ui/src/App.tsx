// The app shell — a real collaboration app over a real session (collaboration scope). The hardcoded
// S2 demo identity (`WS`/`CHANNEL`/`AUTHOR`) is gone: identity now comes from `useSession` (a verified
// token). Logged out → the login screen; logged in → the nav rail + workspace/channel switchers + the
// selected surface. Layout + wiring only; each surface owns its data (FILE-LAYOUT).

import { useState } from "react";

import { useSession, CAP, hasCap, isAdmin } from "@/lib/session";
import { LoginView } from "./features/session";
import { NavRail, type Surface } from "./features/shell";
import { WorkspaceSwitcher } from "./features/workspace";
import { ChannelList, ChannelView } from "./features/channel";
import { MembersView } from "./features/members";
import { InboxView } from "./features/inbox";
import { OutboxView } from "./features/outbox";
import { AdminView } from "./features/admin";
import { ExtensionsView } from "./features/extensions";

export function App() {
  const { session, signIn, signOut } = useSession();
  const [surface, setSurface] = useState<Surface>("channels");
  const [channel, setChannel] = useState("general");

  if (!session) {
    return <LoginView onSignIn={signIn} />;
  }

  const { workspace, principal, caps } = session;
  // Switching workspace is a re-login (the workspace is the token's hard wall §7), keeping identity.
  const switchWorkspace = (ws: string) => void signIn(principal, ws);

  // Cap-gate the admin surfaces' VISIBILITY (admin-console scope). This is convenience only — the
  // gateway re-checks every verb server-side, so a forged call by a non-admin is denied regardless
  // (proven in role/gateway/tests/admin_routes_test.rs). Hiding the controls just avoids dead buttons.
  const allowed: Surface[] = ["channels", "members", "inbox", "outbox"];
  if (isAdmin(caps)) allowed.push("admin");
  if (hasCap(caps, CAP.extList)) allowed.push("extensions");
  const active = allowed.includes(surface) ? surface : "channels";

  return (
    <div className="flex h-full">
      <NavRail active={active} onSelect={setSurface} onSignOut={signOut} allowed={allowed} />

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
        {active === "inbox" && <InboxView ws={workspace} />}
        {active === "outbox" && <OutboxView ws={workspace} />}
        {active === "admin" && <AdminView ws={workspace} caps={caps} />}
        {active === "extensions" && <ExtensionsView ws={workspace} />}
      </main>
    </div>
  );
}
