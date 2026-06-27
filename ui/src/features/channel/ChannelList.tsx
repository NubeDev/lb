// The channel list — the switcher rail showing the workspace's registered channels + a create box
// (collaboration scope, slice 2). Markup + wiring only; data lives in useChannels.

import { useState } from "react";
import { Hash, Plus } from "lucide-react";

import { useChannels } from "./useChannels";

interface Props {
  ws: string;
  /** The currently open channel id. */
  selected: string;
  onSelect: (channel: string) => void;
}

export function ChannelList({ ws, selected, onSelect }: Props) {
  const { channels, error, create } = useChannels(ws);
  const [newChannel, setNewChannel] = useState("");

  return (
    <div className="flex flex-1 flex-col overflow-y-auto px-2 py-3">
      <div className="mb-2 px-1 text-xs font-semibold text-muted">Channels</div>
      {error && (
        <div role="alert" className="mb-2 rounded-md border border-red-500/25 bg-red-500/10 px-2 py-1.5 text-xs text-red-600 dark:text-red-300">
          {error}
        </div>
      )}
      <ul className="flex flex-col gap-1">
        {channels.map((c) => (
          <li key={c.id}>
            <button
              className={`flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-left text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/25 ${
                c.id === selected
                  ? "border-accent/25 bg-accent/15 text-accent shadow-sm shadow-black/5"
                  : "border-transparent text-fg hover:border-border hover:bg-bg"
              }`}
              onClick={() => onSelect(c.id)}
            >
              <Hash size={14} className="shrink-0 text-muted" />
              <span className="truncate">{c.id}</span>
            </button>
          </li>
        ))}
      </ul>
      <form
        className="mt-auto flex gap-1.5 px-1 pt-3"
        onSubmit={(e) => {
          e.preventDefault();
          const ch = newChannel.trim();
          if (ch) {
            void create(ch);
            setNewChannel("");
            onSelect(ch);
          }
        }}
      >
        <input
          aria-label="new channel"
          className="control-field-sm min-w-0 flex-1"
          placeholder="new channel…"
          value={newChannel}
          onChange={(e) => setNewChannel(e.target.value)}
        />
        <button aria-label="create channel" className="soft-button-sm px-2">
          <Plus size={14} />
        </button>
      </form>
    </div>
  );
}
