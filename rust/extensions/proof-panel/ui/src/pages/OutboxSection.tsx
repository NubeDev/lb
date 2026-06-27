import { RefreshCw } from "lucide-react";

import { Button, Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useOutboxStatus } from "@/data/useOutboxStatus";

/** The durable-motion section: a card of `outbox.status` counts (pending / delivered / dead-lettered)
 *  with a Refresh button. Read-only, no args. Honest states — a denied call shows the error, an empty
 *  outbox shows zeros (the truth), never fabricated counts. */
export function OutboxSection() {
  const { state, refresh } = useOutboxStatus();

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>Outbox status</CardTitle>
        <Button
          variant="outline"
          size="sm"
          aria-label="refresh outbox"
          onClick={refresh}
          disabled={state.status === "loading"}
        >
          <RefreshCw className="h-3.5 w-3.5" aria-hidden />
          Refresh
        </Button>
      </CardHeader>
      <CardContent>
        {state.status === "loading" && <p>Reading outbox…</p>}
        {state.status === "error" && (
          <p className="text-accent">Could not read outbox: {state.error}</p>
        )}
        {state.status === "ready" && (
          <dl className="grid grid-cols-3 gap-2" data-testid="outbox-counts">
            <Stat label="Pending" value={state.data.pending.length} testid="outbox-pending" />
            <Stat label="Delivered" value={state.data.delivered.length} testid="outbox-delivered" />
            <Stat
              label="Dead-lettered"
              value={state.data.dead_lettered.length}
              testid="outbox-dead"
            />
          </dl>
        )}
      </CardContent>
    </Card>
  );
}

function Stat({ label, value, testid }: { label: string; value: number; testid: string }) {
  return (
    <div className="rounded-md border border-border p-3 text-center">
      <dt className="text-xs text-muted">{label}</dt>
      <dd className="text-lg font-semibold text-fg" data-testid={testid}>
        {value}
      </dd>
    </div>
  );
}
