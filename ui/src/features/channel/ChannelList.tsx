// The channel list — the switcher rail showing the workspace's registered channels + a create box
// (collaboration scope, slice 2). Markup + wiring only; data lives in useChannels. On shadcn
// primitives (Button/Input/Alert) + tokens (ui-standards-scope), shaped like the canonical FlowRail.

import { useState } from "react";
import { Hash, Plus } from "lucide-react";

import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
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
        <Alert variant="destructive" className="mb-2 px-2 py-1.5 text-xs">
          {error}
        </Alert>
      )}
      <ul className="flex flex-col gap-1">
        {channels.map((c) => {
          const isSelected = c.id === selected;
          return (
            <li key={c.id}>
              <Button
                variant="ghost"
                onClick={() => onSelect(c.id)}
                className={cn(
                  "h-auto w-full justify-start gap-2 border px-2.5 py-1.5 text-left text-sm font-normal",
                  isSelected
                    ? "border-accent/25 bg-accent/15 text-accent shadow-sm shadow-black/5 hover:bg-accent/15"
                    : "border-transparent text-fg hover:border-border hover:bg-bg",
                )}
              >
                <Hash size={14} className="shrink-0 text-muted" />
                <span className="min-w-0 flex-1 truncate">{c.id}</span>
              </Button>
            </li>
          );
        })}
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
        <Input
          aria-label="new channel"
          className="h-8 min-w-0 flex-1 text-xs"
          placeholder="new channel…"
          value={newChannel}
          onChange={(e) => setNewChannel(e.target.value)}
        />
        <Button aria-label="create channel" variant="outline" size="sm" className="h-8 px-2">
          <Plus size={14} />
        </Button>
      </form>
    </div>
  );
}
