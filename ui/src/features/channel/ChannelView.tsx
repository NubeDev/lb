// The channel view — composes the hook + list + composer into one screen, plus a presence roster
// (who's online, collaboration scope slice 3). Data lives in useChannel/usePresence; this file is
// layout + wiring only (FILE-LAYOUT).

import { Hash } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { useChannel } from "./useChannel";
import { usePresence } from "./usePresence";
import { MessageList } from "./MessageList";
import { CommandPalette } from "./palette/CommandPalette";

interface Props {
  ws: string;
  channel: string;
  author: string;
  /** Injected logical clock — kept overridable so tests are deterministic. */
  now?: () => number;
}

export function ChannelView({ ws, channel, author, now }: Props) {
  const { items, loading, error, send, edit, remove, postQuery, callTool } = useChannel(
    ws,
    channel,
    author,
    now,
  );
  const online = usePresence(ws, channel);

  return (
    <section aria-label="channel view" className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Hash}
        title={`#${channel}`}
        description={`Posting as ${author}`}
        workspace={ws}
      />

      {online.length > 0 && (
        <div
          aria-label="online members"
          className="flex flex-wrap items-center gap-2 border-b border-border bg-panel px-4 py-2 text-xs text-muted"
        >
          <span className="font-medium">Online</span>
          {online.map((m) => (
            <span key={m} className="flex items-center gap-1 rounded-full border border-border bg-bg px-2 py-0.5">
              <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" aria-hidden />
              {m}
            </span>
          ))}
        </div>
      )}

      {error && (
        <div
          role="alert"
          aria-label="channel error"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-xs text-destructive"
        >
          {error}
        </div>
      )}

      {loading ? (
        <div className="flex flex-1 flex-col gap-2 p-4" aria-label="loading messages">
          <div className="h-12 w-2/3 animate-pulse rounded-md border border-border bg-panel" />
          <div className="h-12 w-1/2 animate-pulse rounded-md border border-border bg-panel" />
          <div className="h-12 w-3/4 animate-pulse rounded-md border border-border bg-panel" />
        </div>
      ) : (
        <MessageList items={items} author={author} onEdit={edit} onDelete={remove} />
      )}

      <CommandPalette
        channel={channel}
        onPostQuery={postQuery}
        onCallTool={callTool}
        onSendChat={send}
      />
    </section>
  );
}
