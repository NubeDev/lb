// The message list — presentation only (FILE-LAYOUT: one component, no data fetching here;
// the data comes from useChannel). Renders items oldest→newest in the quiet panel style.

import type { Item } from "@/lib/channel/channel.types";

interface Props {
  items: Item[];
}

export function MessageList({ items }: Props) {
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
        <li
          key={m.id}
          className="rounded-md border border-border bg-panel px-3 py-2 text-sm shadow-sm shadow-black/5"
        >
          <div className="mb-1 truncate text-xs font-medium text-accent">{m.author}</div>
          <div className="break-words leading-6 text-fg">{m.body}</div>
        </li>
      ))}
    </ul>
  );
}
