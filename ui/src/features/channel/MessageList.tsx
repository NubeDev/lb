// The message list — presentation only (FILE-LAYOUT: one component, no data fetching here;
// the data comes from useChannel). Renders items oldest→newest in the quiet panel style.

import type { Item } from "@/lib/channel/channel.types";

interface Props {
  items: Item[];
}

export function MessageList({ items }: Props) {
  if (items.length === 0) {
    return (
      <div className="flex flex-1 items-center justify-center text-sm text-muted">
        No messages yet — say something.
      </div>
    );
  }
  return (
    <ul className="flex flex-1 flex-col gap-2 overflow-y-auto p-4" aria-label="messages">
      {items.map((m) => (
        <li
          key={m.id}
          className="rounded-md border border-border bg-panel px-3 py-2 text-sm"
        >
          <span className="mr-2 font-medium text-accent">{m.author}</span>
          <span className="text-fg">{m.body}</span>
        </li>
      ))}
    </ul>
  );
}
