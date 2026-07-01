// The message list — presentation only (FILE-LAYOUT: one component, no data fetching here; the data
// comes from useChannel). Renders items oldest→newest. Each row is a MessageItem; the current
// viewer's own rows get edit/delete affordances (wired through from useChannel via these props).

import type { Item } from "@/lib/channel/channel.types";
import { MessageItem } from "./MessageItem";

interface Props {
  items: Item[];
  /** The current viewer's identity — own messages are editable/deletable. */
  author: string;
  onEdit: (id: string, body: string) => Promise<void> | void;
  onDelete: (id: string) => Promise<void> | void;
}

export function MessageList({ items, author, onEdit, onDelete }: Props) {
  // The run ids that already have a durable answer/error (the agent worker posts those under id
  // `a:<job>`). A pending `agent` request whose job is in here is superseded — AgentCard hides its
  // "running…" placeholder so a completed run never shows a stuck spinner.
  const settledJobs = new Set(
    items.filter((m) => m.id.startsWith("a:")).map((m) => m.id.slice(2)),
  );
  if (items.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center p-6">
        <div className="rounded-lg border border-dashed border-border bg-panel/70 px-5 py-4 text-center">
          <p className="text-sm font-medium text-fg">No messages yet</p>
          <p className="mt-1 text-xs text-muted">Start the channel history with a short update.</p>
        </div>
      </div>
    );
  }
  return (
    <ul className="flex flex-1 flex-col gap-2 overflow-y-auto p-4" aria-label="messages">
      {items.map((m) => (
        <MessageItem
          key={m.id}
          item={m}
          author={author}
          onEdit={onEdit}
          onDelete={onDelete}
          settledJobs={settledJobs}
        />
      ))}
    </ul>
  );
}
