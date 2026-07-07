// The insight action buttons — ack / resolve (insights umbrella scope). Each is gated on the
// caller's caps (the host re-checks server-side; the UI gate is convenience). Mirrors the Inbox
// `DetailPane` footer: shadcn `Button`s with a spinning `RefreshCw` while in flight, the primary
// action (Resolve) in the accent tone, the secondary (Ack) outlined.
//
// NOTE: the button label MUST stay the bare word "Ack" (not "Acking…") — the gateway test finds
// it by accessible name `/^Ack$/`. The busy state is conveyed by the spinner icon next to the
// stable label, the same device Inbox uses — so the assertion still resolves against the same
// element across the click.

import { useState } from "react";
import { Check, CheckCheck, RefreshCw } from "lucide-react";

import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { ackInsight, resolveInsight } from "@/lib/insights/insights.api";
import type { Insight } from "@/lib/insights/insights.types";

interface Props {
  insight: Insight;
  /** Called after an ack/resolve lands so the parent can refresh its view. */
  onActed?: () => void;
}

/** The ack/resolve row for the detail pane. Ack is hidden once acked/resolved; resolve once
 *  resolved — the status-driven visibility so a stale action can't be re-fired. */
export function InsightActions({ insight, onActed }: Props): JSX.Element {
  const [busy, setBusy] = useState<"ack" | "resolve" | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function onAck() {
    setBusy("ack");
    setError(null);
    try {
      await ackInsight(insight.id);
      onActed?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  }

  async function onResolve() {
    setBusy("resolve");
    setError(null);
    try {
      await resolveInsight(insight.id);
      onActed?.();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  }

  const disabled = busy !== null;

  return (
    <div className="flex w-full flex-col gap-2">
      <div className="flex items-center justify-end gap-2 border-t border-border pt-3">
        {insight.status === "open" && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={onAck}
            disabled={disabled}
          >
            {busy === "ack" ? (
              <RefreshCw size={14} className={cn("animate-spin")} />
            ) : (
              <Check size={14} />
            )}
            Ack
          </Button>
        )}
        {insight.status !== "resolved" && (
          <Button
            type="button"
            variant="default"
            size="sm"
            onClick={onResolve}
            disabled={disabled}
          >
            {busy === "resolve" ? (
              <RefreshCw size={14} className={cn("animate-spin")} />
            ) : (
              <CheckCheck size={14} />
            )}
            Resolve
          </Button>
        )}
        {insight.status === "resolved" && (
          <span className="inline-flex items-center gap-1.5 text-xs text-success">
            <CheckCheck size={14} />
            Resolved
          </span>
        )}
      </div>

      {error && (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      )}
    </div>
  );
}
