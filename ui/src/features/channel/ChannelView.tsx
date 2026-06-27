// The channel view — composes the hook + list + composer into one screen, plus a presence roster
// (who's online, collaboration scope slice 3). Data lives in useChannel/usePresence; this file is
// layout + wiring only (FILE-LAYOUT).

import { Hash } from "lucide-react";

import { useChannel } from "./useChannel";
import { usePresence } from "./usePresence";
import { MessageList } from "./MessageList";
import { MessageComposer } from "./MessageComposer";

interface Props {
  ws: string;
  channel: string;
  author: string;
  /** Injected logical clock — kept overridable so tests are deterministic. */
  now?: () => number;
}

export function ChannelView({ ws, channel, author, now }: Props) {
  const { items, loading, error, send } = useChannel(ws, channel, author, now);
  const online = usePresence(ws, channel);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Hash size={16} className="text-muted" />
        <h1 className="text-sm font-medium">{channel}</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {online.length > 0 && (
        <div
          aria-label="online members"
          className="flex flex-wrap items-center gap-2 border-b border-border bg-panel px-4 py-1.5 text-xs text-muted"
        >
          <span className="font-medium">Online</span>
          {online.map((m) => (
            <span key={m} className="flex items-center gap-1">
              <span className="h-1.5 w-1.5 rounded-full bg-green-500" aria-hidden />
              {m}
            </span>
          ))}
        </div>
      )}

      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error}
        </div>
      )}

      {loading ? (
        <div className="flex flex-1 items-center justify-center text-sm text-muted">
          Loading…
        </div>
      ) : (
        <MessageList items={items} />
      )}

      <MessageComposer onSend={send} />
    </section>
  );
}
