// The app shell (S2 minimal): a compact rail + the channel view. The rail is a placeholder
// for the workspace switcher / nav that fills in as the backend surface grows (frontend
// scope keeps P0 small). One workspace + one channel is enough to prove the messaging slice.

import { MessageSquare } from "lucide-react";

import { ChannelView } from "./features/channel";

// S2 demo identity. At S3 these come from the session (a verified principal over IPC).
const WS = "acme";
const CHANNEL = "general";
const AUTHOR = "user:me";

export function App() {
  return (
    <div className="flex h-full">
      <nav className="flex w-12 flex-col items-center gap-3 border-r border-border bg-panel py-3">
        <div className="rounded-md bg-accent/15 p-2 text-accent" title="Channels">
          <MessageSquare size={18} />
        </div>
      </nav>
      <main className="flex-1">
        <ChannelView ws={WS} channel={CHANNEL} author={AUTHOR} />
      </main>
    </div>
  );
}
