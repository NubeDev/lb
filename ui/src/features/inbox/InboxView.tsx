// The inbox view — the real triage/approval queue (collaboration scope, slice 4). Lists durable
// `lb-inbox` items and offers Approve / Reject on each (the S6 approval gate as a real UI action).
// Markup + wiring only; data lives in useInbox.

import { Inbox } from "lucide-react";

import { useInbox } from "./useInbox";

interface Props {
  /** The inbox channel to triage (e.g. `approvals`). */
  channel?: string;
  ws: string;
}

export function InboxView({ channel = "approvals", ws }: Props) {
  const { items, error, resolve } = useInbox(channel);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="page-header">
        <div className="page-header-icon">
          <Inbox size={16} />
        </div>
        <div className="min-w-0">
          <h1 className="page-title">Inbox</h1>
          <p className="page-subtitle">Triage queue: {channel}</p>
        </div>
        <span className="scope-pill ml-auto" title={`Workspace ${ws}`}>
          <span className="h-1.5 w-1.5 rounded-full bg-accent" aria-hidden />
          <span className="truncate">{ws}</span>
        </span>
      </header>

      {error && (
        <div role="alert" className="state-alert">
          {error}
        </div>
      )}

      <ul className="flex-1 overflow-y-auto px-4 py-2">
        {items.length === 0 ? (
          <li className="text-sm text-muted">No items.</li>
        ) : (
          items.map((it) => (
            <li
              key={it.id}
              role="listitem"
              className="flex items-center gap-2 border-b border-border py-2"
            >
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm">{it.body}</div>
                <div className="text-xs text-muted">
                  {it.author} · {it.id}
                </div>
              </div>
              <button
                aria-label={`approve ${it.id}`}
                className="rounded bg-green-500/15 px-2 py-1 text-xs text-green-500"
                onClick={() => void resolve(it.id, "approved")}
              >
                Approve
              </button>
              <button
                aria-label={`reject ${it.id}`}
                className="rounded bg-accent/15 px-2 py-1 text-xs text-accent"
                onClick={() => void resolve(it.id, "rejected")}
              >
                Reject
              </button>
            </li>
          ))
        )}
      </ul>
    </section>
  );
}
