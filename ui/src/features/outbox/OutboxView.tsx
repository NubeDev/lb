// The outbox status view — read-only delivery status (collaboration scope, slice 4). Shows the
// workspace's effects grouped pending → delivered (→ dead-letter). No editing: the outbox is
// must-deliver infrastructure, users see effects + status, never an editable queue. Data in useOutbox.

import { Send } from "lucide-react";

import { useOutbox } from "./useOutbox";
import type { Effect } from "@/lib/outbox/outbox.types";

interface Props {
  ws: string;
}

function Group({ title, effects }: { title: string; effects: Effect[] }) {
  return (
    <div className="px-4 py-2">
      <div className="mb-1 text-xs font-medium text-muted">
        {title} · {effects.length}
      </div>
      <ul>
        {effects.map((e) => (
          <li key={e.id} role="listitem" className="py-1 text-sm">
            <span className="text-muted">{e.target}</span> {e.action}{" "}
            <span className="text-xs text-muted">({e.status}, {e.attempts} attempts)</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

export function OutboxView({ ws }: Props) {
  const { status, error } = useOutbox();

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Send size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Outbox</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error}
        </div>
      )}

      <div className="flex-1 overflow-y-auto divide-y divide-border">
        <Group title="Pending" effects={status.pending} />
        <Group title="Delivered" effects={status.delivered} />
        <Group title="Dead-lettered" effects={status.dead_lettered} />
      </div>
    </section>
  );
}
