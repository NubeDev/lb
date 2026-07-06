// The message list — presentation only (FILE-LAYOUT: one component, no data fetching here; the data
// comes from useChannel). Renders items oldest→newest. Each row is a MessageItem; the current
// viewer's own rows get edit/delete affordances (wired through from useChannel via these props).

import type { Item } from "@/lib/channel/channel.types";
import type { ExtRow } from "@/lib/ext/ext.api";
import { MessageItem } from "./MessageItem";

interface Props {
  items: Item[];
  /** The current viewer's identity — own messages are editable/deletable. */
  author: string;
  /** The viewer's session workspace — threaded to a `rich_result`'s mounted widget. */
  ws: string;
  /** Installed extensions (from `ext.list`) — threaded to a `rich_result`'s ext response view. */
  installed?: ExtRow[];
  onEdit: (id: string, body: string) => Promise<void> | void;
  onDelete: (id: string) => Promise<void> | void;
  /** Optional gather-as-context seam (agent-context-basket scope) — threaded to each row; the dock
   *  passes it, the channel view doesn't (absent → no affordance, unchanged). */
  contextAction?: { has: (id: string) => boolean; toggle: (id: string) => void };
  /** Optional query rerun/edit seam (query re-edit scope) — threaded to each row's QueryCard. */
  queryActions?: import("./query/QueryCard").QueryActions;
}

export function MessageList({
  items,
  author,
  ws,
  installed,
  onEdit,
  onDelete,
  contextAction,
  queryActions,
}: Props) {
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
          ws={ws}
          installed={installed}
          onEdit={onEdit}
          onDelete={onDelete}
          settledJobs={settledJobs}
          contextAction={contextAction}
          queryActions={queryActions}
        />
      ))}
    </ul>
  );
}
