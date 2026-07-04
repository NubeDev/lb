// The inbox view — the real triage/approval queue (collaboration scope, slice 4). Lists durable
// `lb-inbox` items and offers Approve / Reject on each (the S6 approval gate as a real UI action).
//
// Master–detail over the Lazybones shadcn primitives (the closest thing in the codebase to the
// shadcn "Mail" demo block — a scrollable list + a reading pane). Markup + selection only; data,
// in-flight state, and per-item errors live in `useInbox`. Approve/Reject failures used to vanish
// into a single banner; they now surface on the item that was clicked, and the buttons disable +
// spin while the resolve is in flight so a slow/rejected click never reads as "nothing happened".

import { useEffect, useState } from "react";
import { Check, Inbox as InboxIcon, RefreshCw, X } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardFooter,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";
import { useInbox } from "./useInbox";

interface Props {
  /** The inbox channel to triage (e.g. `approvals`). */
  channel?: string;
  ws: string;
}

export function InboxView({ channel = "approvals", ws }: Props) {
  const { items, error, loading, resolving, errors, refresh, resolve } = useInbox(channel);

  // The item currently shown in the reading pane. Auto-follows the list: defaults to the first
  // item, and after a resolve refresh re-points to the new head when the resolved one is gone.
  const [selectedId, setSelectedId] = useState<string | null>(null);
  useEffect(() => {
    if (items.length === 0) {
      if (selectedId !== null) setSelectedId(null);
      return;
    }
    if (!selectedId || !items.some((it) => it.id === selectedId)) {
      setSelectedId(items[0].id);
    }
  }, [items, selectedId]);

  const selected = items.find((it) => it.id === selectedId) ?? null;

  return (
    <section className="flex h-full flex-col bg-bg">
      <AppPageHeader
        icon={InboxIcon}
        title="Inbox"
        description={`Triage queue: ${channel}`}
        workspace={ws}
        actions={
          <Button
            variant="outline"
            size="sm"
            onClick={() => void refresh()}
            disabled={loading}
            aria-label="Refresh inbox"
          >
            <RefreshCw size={14} className={cn(loading && "animate-spin")} />
            Refresh
          </Button>
        }
      />

      {error && !selected && (
        <div className="px-4 pt-3">
          <Alert variant="destructive">
            <AlertTitle>Couldn’t load the inbox</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        </div>
      )}

      {items.length === 0 ? (
        <EmptyPane loading={loading} />
      ) : (
        <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[20rem_1fr]">
          {/* Master: the scrollable triage list. */}
          <ul
            role="list"
            className="min-h-0 overflow-y-auto border-b border-border md:border-b-0 md:border-r"
          >
            {items.map((it) => {
              const active = it.id === selectedId;
              return (
                <li key={it.id} role="listitem">
                  <button
                    type="button"
                    onClick={() => setSelectedId(it.id)}
                    aria-current={active ? "true" : undefined}
                    className={cn(
                      "flex w-full items-start gap-3 border-l-2 px-4 py-3 text-left transition-colors",
                      active
                        ? "border-l-accent bg-accent/10"
                        : "border-l-transparent hover:bg-panel",
                    )}
                  >
                    <span
                      className={cn(
                        "mt-1.5 h-2 w-2 shrink-0 rounded-full",
                        active ? "bg-accent" : "bg-muted/40",
                      )}
                      aria-hidden
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm text-fg">{it.body}</span>
                      <span className="mt-0.5 block truncate text-xs text-muted">
                        {it.author} · {it.id}
                      </span>
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>

          {/* Detail: the reading pane for the selected item. */}
          <div className="min-h-0 overflow-y-auto p-4">
            {selected ? (
              <DetailPane
                body={selected.body}
                author={selected.author}
                id={selected.id}
                ts={selected.ts}
                busy={resolving === selected.id}
                resolveDisabled={resolving !== null}
                itemError={errors[selected.id] || null}
                onResolve={(decision) => void resolve(selected.id, decision)}
              />
            ) : (
              <p className="text-sm text-muted">Select an item to review.</p>
            )}
          </div>
        </div>
      )}
    </section>
  );
}

/** The reading pane — the full item body plus the Approve/Reject actions (the S6 gate as a UI action). */
function DetailPane({
  body,
  author,
  id,
  ts,
  busy,
  resolveDisabled,
  itemError,
  onResolve,
}: {
  body: string;
  author: string;
  id: string;
  ts: number;
  busy: boolean;
  resolveDisabled: boolean;
  itemError: string | null;
  onResolve: (decision: "approved" | "rejected") => void;
}) {
  return (
    <Card>
      <CardHeader>
        <div className="flex items-center justify-between gap-2">
          <CardTitle className="text-base">{author}</CardTitle>
          <Badge variant="outline" className="font-mono text-[10px]">
            {id}
          </Badge>
        </div>
        <p className="text-xs text-muted">
          {ts > 0 ? new Date(ts).toLocaleString() : "pending"}
        </p>
      </CardHeader>
      <CardContent>
        <p className="whitespace-pre-wrap text-sm text-fg">{body}</p>
      </CardContent>

      {itemError && (
        <div className="px-4">
          <Alert variant="destructive">
            <AlertTitle>Couldn’t resolve this item</AlertTitle>
            <AlertDescription>{itemError}</AlertDescription>
          </Alert>
        </div>
      )}

      <CardFooter className="gap-2">
        <Button
          variant="default"
          size="sm"
          onClick={() => onResolve("approved")}
          disabled={resolveDisabled}
          aria-label={`approve ${id}`}
        >
          {busy ? <RefreshCw size={14} className="animate-spin" /> : <Check size={14} />}
          Approve
        </Button>
        <Button
          variant="destructive"
          size="sm"
          onClick={() => onResolve("rejected")}
          disabled={resolveDisabled}
          aria-label={`reject ${id}`}
        >
          {busy ? <RefreshCw size={14} className="animate-spin" /> : <X size={14} />}
          Reject
        </Button>
      </CardFooter>
    </Card>
  );
}

function EmptyPane({ loading }: { loading: boolean }) {
  return (
    <div className="flex flex-1 items-center justify-center p-6">
      <Card className="w-full max-w-sm">
        <CardContent className="flex items-center gap-3 p-4 text-sm text-muted">
          <InboxIcon size={18} className={cn(loading && "animate-spin")} />
          {loading ? "Loading the queue…" : "No items in this queue."}
        </CardContent>
      </Card>
    </div>
  );
}
