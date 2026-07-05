// The insight action buttons — ack / resolve (insights umbrella scope). Each is gated on the
// caller's caps (the host re-checks server-side; the UI gate is convenience). Disable + spin on
// in-flight (the inbox `resolving` pattern).
//
// STUB: the buttons render + call the api; per-row error surfacing + the cap-driven visibility
// (hide ack on an already-acked insight; hide resolve on a resolved one) are TODO.

import { useState } from "react";

import { ackInsight, resolveInsight } from "@/lib/insights/insights.api";
import type { Insight } from "@/lib/insights/insights.types";

interface Props {
  insight: Insight;
}

/** The ack/resolve row for the detail drawer. */
export function InsightActions({ insight }: Props): JSX.Element {
  const [busy, setBusy] = useState<"ack" | "resolve" | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function onAck() {
    setBusy("ack");
    setError(null);
    try {
      await ackInsight(insight.id);
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
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="flex items-center gap-2 border-t border-border pt-3">
      <button
        type="button"
        onClick={onAck}
        disabled={busy !== null || insight.status === "resolved"}
        className="rounded-md border border-border px-3 py-1 text-sm disabled:opacity-50"
      >
        {busy === "ack" ? "Acking…" : "Ack"}
      </button>
      <button
        type="button"
        onClick={onResolve}
        disabled={busy !== null || insight.status === "resolved"}
        className="rounded-md border border-border px-3 py-1 text-sm disabled:opacity-50"
      >
        {busy === "resolve" ? "Resolving…" : "Resolve"}
      </button>
      {error && <span className="text-xs text-destructive">{error}</span>}
    </section>
  );
}
