import { Check, RefreshCw, X } from "lucide-react";

import { Button, Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useInboxList } from "@/data/useInboxList";
import { useInboxResolve } from "@/data/useInboxResolve";
import type { Decision } from "@/data/workflow.types";

/** The channel this demo triages. The node may produce no items for it — section shows an HONEST empty
 *  list rather than fabricating workflow state (proof-panel scope open question 1, option (a)). */
const TRIAGE_CHANNEL = "triage";

/** The durable-workflow section: `inbox.list { channel }` items, each with Approve/Reject calling
 *  `inbox.resolve { item_id, decision }` — the page's first WRITE that mutates durable workflow state.
 *  Honest empty/error states throughout. */
export function InboxSection() {
  const list = useInboxList(TRIAGE_CHANNEL);
  const resolver = useInboxResolve();

  async function decide(itemId: string, decision: Decision) {
    // A monotone-ish logical ts for ordering; the resolution is idempotent on the item id regardless.
    const ok = await resolver.resolve(itemId, decision, Date.now());
    if (ok) list.refresh();
  }

  const busy = resolver.state.status === "resolving";

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>Inbox triage</CardTitle>
        <Button
          variant="outline"
          size="sm"
          aria-label="refresh inbox"
          onClick={list.refresh}
          disabled={list.state.status === "loading"}
        >
          <RefreshCw className="h-3.5 w-3.5" aria-hidden />
          Refresh
        </Button>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <p className="text-xs text-muted">
          Items on <span className="text-fg">{TRIAGE_CHANNEL}</span> via <code>inbox.list</code>;
          Approve/Reject writes a resolution via <code>inbox.resolve</code>.
        </p>

        {resolver.state.status === "error" && (
          <p className="text-accent">Could not resolve: {resolver.state.error}</p>
        )}

        {list.state.status === "loading" && <p>Reading inbox…</p>}
        {list.state.status === "error" && (
          <p className="text-accent">Could not read inbox: {list.state.error}</p>
        )}
        {list.state.status === "ready" && list.state.data.length === 0 && (
          <p data-testid="inbox-empty">No items to triage on this channel.</p>
        )}
        {list.state.status === "ready" && list.state.data.length > 0 && (
          <ul className="divide-y divide-border" data-testid="inbox-list">
            {list.state.data.map((item) => (
              <li key={item.id} className="flex items-center justify-between gap-2 py-2">
                <span className="min-w-0 flex-1 truncate text-fg" title={item.body}>
                  {item.body || item.id}
                </span>
                <div className="flex gap-1.5">
                  <Button
                    size="sm"
                    aria-label={`approve ${item.id}`}
                    onClick={() => decide(item.id, "approved")}
                    disabled={busy}
                  >
                    <Check className="h-3.5 w-3.5" aria-hidden />
                    Approve
                  </Button>
                  <Button
                    variant="outline"
                    size="sm"
                    aria-label={`reject ${item.id}`}
                    onClick={() => decide(item.id, "rejected")}
                    disabled={busy}
                  >
                    <X className="h-3.5 w-3.5" aria-hidden />
                    Reject
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}
