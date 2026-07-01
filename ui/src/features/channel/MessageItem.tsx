// A single channel message — presentation + the author's own-message edit/delete affordances
// (FILE-LAYOUT: one component per file). Own messages (item.author === the viewer's `author`) get
// hover-revealed Edit / Delete controls; editing is an inline Input that calls back into
// `useChannel::edit`. Kind-tagged payloads (query/result/error) are not text-editable — only
// deletable — so the Edit button is hidden for them.

import { useEffect, useRef, useState } from "react";
import { Check, Pencil, Trash2, X } from "lucide-react";

import type { Item } from "@/lib/channel/channel.types";
import { parsePayload } from "@/lib/channel/payload.types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { QueryCard } from "./query/QueryCard";
import { AgentCard } from "./AgentCard";

interface Props {
  item: Item;
  /** The current viewer's identity (`Item.author` shape) — own messages are editable/deletable. */
  author: string;
  onEdit: (id: string, body: string) => Promise<void> | void;
  onDelete: (id: string) => Promise<void> | void;
  /** Run ids that already have a durable answer/error (from MessageList) — a settled `agent` request
   *  hides its "running…" placeholder. */
  settledJobs?: Set<string>;
}

/** The agent payload kinds route to AgentCard; everything else kind-tagged goes to QueryCard. */
function isAgentPayload(
  p: ReturnType<typeof parsePayload>,
): p is Extract<NonNullable<ReturnType<typeof parsePayload>>, { kind: `agent${string}` }> {
  return !!p && (p.kind === "agent" || p.kind === "agent_result" || p.kind === "agent_error");
}

export function MessageItem({ item, author, onEdit, onDelete, settledJobs }: Props) {
  const payload = parsePayload(item.body);
  const isOwn = item.author === author;
  // Edit applies to plain chat text only — a kind-tagged payload is structured, not free text.
  const canEdit = isOwn && !payload;

  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(item.body);
  const [busy, setBusy] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  // Reset the draft whenever we enter/leave edit mode so stale text never submits.
  useEffect(() => {
    if (editing) {
      setDraft(item.body);
      inputRef.current?.focus();
    }
  }, [editing, item.body]);

  async function save() {
    const trimmed = draft.trim();
    if (!trimmed || trimmed === item.body) {
      setEditing(false);
      return;
    }
    setBusy(true);
    try {
      await onEdit(item.id, trimmed);
      setEditing(false);
    } finally {
      setBusy(false);
    }
  }

  function cancel() {
    setDraft(item.body);
    setEditing(false);
  }

  async function remove() {
    if (!window.confirm("Delete this message?")) return;
    setBusy(true);
    try {
      await onDelete(item.id);
    } finally {
      setBusy(false);
    }
  }

  return (
    <li className="group rounded-md border border-border bg-panel px-3 py-2 text-sm shadow-sm shadow-black/5">
      <div className="mb-1 flex items-center gap-2">
        <div className="truncate text-xs font-medium text-accent">{item.author}</div>
        {isOwn && !editing && (
          <div className="ml-auto flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
            {canEdit && (
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7"
                aria-label="Edit message"
                title="Edit"
                onClick={() => setEditing(true)}
              >
                <Pencil size={14} />
              </Button>
            )}
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7 text-destructive hover:text-destructive"
              aria-label="Delete message"
              title="Delete"
              disabled={busy}
              onClick={remove}
            >
              <Trash2 size={14} />
            </Button>
          </div>
        )}
      </div>

      {editing ? (
        <div className="flex items-center gap-1.5">
          <Input
            ref={inputRef}
            value={draft}
            disabled={busy}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void save();
              else if (e.key === "Escape") cancel();
            }}
            aria-label="Edit message body"
          />
          <Button
            variant="default"
            size="icon"
            className="h-9 w-9"
            aria-label="Save edit"
            title="Save"
            disabled={busy}
            onClick={() => void save()}
          >
            <Check size={16} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-9 w-9"
            aria-label="Cancel edit"
            title="Cancel"
            disabled={busy}
            onClick={cancel}
          >
            <X size={16} />
          </Button>
        </div>
      ) : isAgentPayload(payload) ? (
        <AgentCard
          payload={payload}
          settled={payload.kind === "agent" && !!settledJobs?.has(payload.job)}
        />
      ) : payload ? (
        <QueryCard payload={payload} channel={item.channel} itemId={item.id} />
      ) : (
        <div className="break-words leading-6 text-fg">{item.body}</div>
      )}
    </li>
  );
}
