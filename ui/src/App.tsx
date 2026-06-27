// The app shell — a real collaboration app over a real session (collaboration scope). The hardcoded
// S2 demo identity (`WS`/`CHANNEL`/`AUTHOR`) is gone: identity now comes from `useSession` (a verified
// token). Logged out → the login screen; logged in → the nav rail + workspace/channel switchers + the
// selected surface. Layout + wiring only; each surface owns its data (FILE-LAYOUT).

import { useState } from "react";

import { useSession } from "@/lib/session";
import { LoginView } from "./features/session";
import { NavRail, type Surface } from "./features/shell";
import { WorkspaceSwitcher } from "./features/workspace";
import { ChannelList, ChannelView } from "./features/channel";
import { MembersView } from "./features/members";
import { InboxView } from "./features/inbox";
import { OutboxView } from "./features/outbox";

export function App() {
  const { session, signIn, signOut } = useSession();
  const [surface, setSurface] = useState<Surface>("channels");
  const [channel, setChannel] = useState("general");

  if (!session) {
    return <LoginView onSignIn={signIn} />;
  }

  const { workspace, principal } = session;
  // Switching workspace is a re-login (the workspace is the token's hard wall §7), keeping identity.
  const switchWorkspace = (ws: string) => void signIn(principal, ws);

  return (
    <div className="flex h-full">
      <NavRail active={surface} onSelect={setSurface} onSignOut={signOut} />

      {surface === "channels" && (
        <aside className="flex w-56 flex-col border-r border-border bg-panel">
          <WorkspaceSwitcher current={workspace} onSwitch={switchWorkspace} />
          <ChannelList ws={workspace} selected={channel} onSelect={setChannel} />
        </aside>
      )}

      <main className="flex-1">
        {surface === "channels" && (
          <ChannelView ws={workspace} channel={channel} author={principal} />
        )}
        {surface === "members" && <MembersView ws={workspace} />}
        {surface === "inbox" && <InboxView ws={workspace} />}
        {surface === "outbox" && <OutboxView ws={workspace} />}
      </main>
    </div>
  );
}
