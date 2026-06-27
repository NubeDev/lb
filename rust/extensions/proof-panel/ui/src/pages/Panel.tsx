import { ShieldCheck } from "lucide-react";

import { useCtx } from "@/app/useCtx";
import { IngestSection } from "./IngestSection";
import { InboxSection } from "./InboxSection";
import { OutboxSection } from "./OutboxSection";
import { SeriesSection } from "./SeriesSection";

/** The single page `proof-panel` contributes — the "whole platform, one page" demo. It proves the
 *  platform end to end from inside ONE cap-gated federated page, through the host-mediated bridge:
 *    1. Ingest → read round-trip — the page CREATES the data it shows (ingest.write → series.latest).
 *    2. Outbox status — the durable-motion snapshot (outbox.status).
 *    3. Inbox triage — the first WRITE that mutates workflow state (inbox.list → inbox.resolve).
 *    + Browse series — the original READ half (series.find → series.latest).
 *  Data is reached ONLY through `bridge` (the host re-checks every call); the workspace badge proves the
 *  host `ctx` (the hard tenant wall) reached the mounted remote. This file is a THIN composition — each
 *  section owns one concern (FILE-LAYOUT). */
export function Panel() {
  const { workspace } = useCtx();

  return (
    <div className="min-h-full bg-bg p-6">
      <div className="mx-auto flex max-w-3xl flex-col gap-4">
        <header className="flex items-center gap-2">
          <ShieldCheck className="h-5 w-5 text-accent" aria-hidden />
          <h1 className="text-lg font-semibold text-fg">Proof Panel</h1>
          <span
            className="ml-1 rounded bg-border/40 px-1.5 py-0.5 text-xs font-normal text-muted"
            aria-label="workspace"
          >
            {workspace}
          </span>
        </header>

        <IngestSection />
        <OutboxSection />
        <InboxSection />
        <SeriesSection />
      </div>
    </div>
  );
}
