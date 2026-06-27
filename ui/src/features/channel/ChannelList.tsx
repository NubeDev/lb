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
    <div className="flex flex-1 flex-col overflow-y-auto px-2 py-2">
      <div className="mb-1 px-1 text-xs font-medium text-muted">Channels</div>
      {error && (
        <div role="alert" className="px-1 text-xs text-accent">
          {error}
        </div>
      )}
      <ul className="flex flex-col gap-0.5">
        {channels.map((c) => (
          <li key={c.id}>
            <button
              className={`flex w-full items-center gap-1 rounded px-2 py-1 text-left text-sm ${
                c.id === selected ? "bg-accent/15 text-accent" : "hover:bg-panel"
              }`}
              onClick={() => onSelect(c.id)}
            >
              <Hash size={14} className="text-muted" />
              {c.id}
            </button>
          </li>
        ))}
      </ul>
      <form
        className="mt-2 flex gap-1 px-1"
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
          className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-xs"
          placeholder="new channel…"
          value={newChannel}
          onChange={(e) => setNewChannel(e.target.value)}
        />
        <button aria-label="create channel" className="rounded bg-accent/15 px-2 text-accent">
          <Plus size={14} />
        </button>
      </form>
    </div>
  );
}
