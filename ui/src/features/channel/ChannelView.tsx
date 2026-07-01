// The channel surface — the canonical `AppPage` (full-width header over a body row), with the
// channel rail (workspace switcher + channel list) on the left and the message panel on the right,
// matching Flows/Dashboard. Data lives in useChannel/usePresence; this file is layout + wiring only
// (FILE-LAYOUT).

import { Hash } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { WorkspaceSwitcher } from "@/features/workspace";
import { useExtensions } from "@/features/extensions/useExtensions";
import { useChannel } from "./useChannel";
import { usePresence } from "./usePresence";
import { ChannelList } from "./ChannelList";
import { MessageList } from "./MessageList";
import { CommandPalette } from "./palette/CommandPalette";

interface Props {
  ws: string;
  channel: string;
  author: string;
  /** Injected logical clock — kept overridable so tests are deterministic. */
  now?: () => number;
  /** Switch the open channel (URL navigate). */
  onSelectChannel?: (channel: string) => void;
  /** Switch the session workspace (re-login). */
  onSwitchWorkspace?: (ws: string) => void;
}

const noop = () => {};

export function ChannelView({ ws, channel, author, now, onSelectChannel, onSwitchWorkspace }: Props) {
  const { items, loading, error, send, edit, remove, postQuery, postAgent, callTool, postRich } =
    useChannel(ws, channel, author, now);
  const online = usePresence(ws, channel);
  // Installed extensions — threaded to the message list so an `ext:<id>` rich_result response view mounts
  // the extension's real tile (the response-side ext widget path).
  const { rows: installed } = useExtensions();

  return (
    <AppPage
      label="channel view"
      icon={Hash}
      title={`#${channel}`}
      description={`Posting as ${author}`}
      workspace={ws}
      error={error}
    >
      <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-panel shadow-sm shadow-black/5">
        <WorkspaceSwitcher current={ws} principal={author} onSwitch={onSwitchWorkspace ?? noop} />
        <ChannelList ws={ws} selected={channel} onSelect={onSelectChannel ?? noop} />
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        {online.length > 0 && (
          <div
            aria-label="online members"
            className="flex flex-wrap items-center gap-2 border-b border-border bg-panel px-4 py-2 text-xs text-muted"
          >
            <span className="font-medium">Online</span>
            {online.map((m) => (
              <span
                key={m}
                className="flex items-center gap-1 rounded-full border border-border bg-bg px-2 py-0.5"
              >
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" aria-hidden />
                {m}
              </span>
            ))}
          </div>
        )}

        {loading ? (
          <div className="flex flex-1 flex-col gap-2 p-4" aria-label="loading messages">
            <div className="h-12 w-2/3 animate-pulse rounded-md border border-border bg-panel" />
            <div className="h-12 w-1/2 animate-pulse rounded-md border border-border bg-panel" />
            <div className="h-12 w-3/4 animate-pulse rounded-md border border-border bg-panel" />
          </div>
        ) : (
          <MessageList items={items} author={author} ws={ws} installed={installed} onEdit={edit} onDelete={remove} />
        )}

        <CommandPalette
          channel={channel}
          onPostQuery={postQuery}
          onSendAgent={postAgent}
          onCallTool={callTool}
          onPostRich={postRich}
          onSendChat={send}
        />
      </div>
    </AppPage>
  );
}
