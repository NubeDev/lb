import { Workflow } from "lucide-react";

import { Button, Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useSimulate } from "@/data/useSimulate";

/** The channel the guest's `proof.simulate` produces its triage item on (must mirror the wasm guest and
 *  the InboxSection that displays it). */
const TRIAGE_CHANNEL = "proof-triage";

/** The "I can finally see it work" proof (proof-workflow-sim scope). "Run workflow simulation" invokes
 *  the extension's OWN backend tool `proof-panel.proof.simulate`: the wasm guest DRIVES a full
 *  inbox→approval→outbox round-trip — records an item on `proof-triage`, resolves it Approved, enqueues
 *  an outbox effect — entirely through the host-mediated callback, under `caller ∩ grant`. On success it
 *  fires `onSimulated` so the page's InboxSection + OutboxSection refresh and the user SEES the produced
 *  item appear in the inbox and the effect appear in the outbox counts. A denied step shows the honest
 *  error, never a fabricated summary. */
export function SimulateSection({ onSimulated }: { onSimulated?: () => void }) {
  const { state, simulate } = useSimulate();

  async function run() {
    const result = await simulate();
    if (result !== null) onSimulated?.();
  }

  const busy = state.status === "running";

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>Run workflow simulation</CardTitle>
        <Button size="sm" aria-label="run workflow simulation" onClick={run} disabled={busy}>
          <Workflow className="h-3.5 w-3.5" aria-hidden />
          {busy ? "Simulating…" : "Run workflow simulation"}
        </Button>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <p className="text-xs text-muted">
          The wasm guest drives a full round-trip through the host callback: <code>inbox.record</code>{" "}
          an item on <span className="text-fg">{TRIAGE_CHANNEL}</span> → <code>inbox.resolve</code>{" "}
          Approved → <code>outbox.enqueue</code> an effect. Watch the inbox and outbox below update.
        </p>

        {state.status === "error" && (
          <p className="text-accent" data-testid="simulate-error">
            Could not simulate: {state.error}
          </p>
        )}
        {state.status === "ok" && (
          <ol className="flex flex-col gap-1 text-fg" data-testid="simulate-result">
            <li>
              ✓ Inbox item created: <span className="text-fg">{state.result.inbox_id}</span>
            </li>
            <li>✓ Resolved: {state.result.resolved ? "Approved" : "—"}</li>
            <li>
              ✓ Outbox effect enqueued · pending now{" "}
              <span className="text-fg">{state.result.outbox_pending}</span>
            </li>
          </ol>
        )}
      </CardContent>
    </Card>
  );
}
