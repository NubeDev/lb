import { useState } from "react";
import { PlusCircle } from "lucide-react";

import { Button, Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useIngestWrite } from "@/data/useIngestWrite";
import { useSeriesLatest } from "@/data/useSeriesLatest";

/** The series this demo writes to and reads back — fixed, so one click proves the whole round-trip. */
const DEMO_SERIES = "proof.demo";

/** The headline section: the page CREATES the data it shows. "Write sample" calls `ingest.write` with
 *  an auto-incrementing `seq` (last committed seq + 1, falling back to 1) and a fresh value, then the
 *  read-back of `series.latest` re-runs to show write → stage → drain → read end to end, in the browser,
 *  through the host-mediated bridge. Honest states throughout — a denied write/read shows the error,
 *  never a fabricated value. */
export function IngestSection() {
  const writer = useIngestWrite();
  const latest = useSeriesLatest(DEMO_SERIES);
  const [lastWritten, setLastWritten] = useState<number | null>(null);

  // Next seq: one past the last committed sample (auto from latest), else 1 — so the demo is one click.
  function nextSeq(): number {
    const cur =
      latest.state.status === "ready" && latest.state.data ? Number(latest.state.data.seq ?? 0) : 0;
    return (Number.isFinite(cur) ? cur : 0) + 1;
  }

  async function writeSample() {
    const seq = nextSeq();
    // A deterministic-yet-changing value so a fresh read visibly reflects the new write.
    const value = Math.round((20 + seq) * 10) / 10;
    const accepted = await writer.write([{ series: DEMO_SERIES, ts: seq, seq, value }]);
    if (accepted && accepted > 0) {
      setLastWritten(value);
      // The node's drain worker commits staging → `series`; re-read so the new value renders live.
      latest.refresh();
    }
  }

  const busy = writer.state.status === "writing";

  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>Ingest → read round-trip</CardTitle>
        <Button
          size="sm"
          aria-label="write sample"
          onClick={writeSample}
          disabled={busy}
        >
          <PlusCircle className="h-3.5 w-3.5" aria-hidden />
          {busy ? "Writing…" : "Write sample"}
        </Button>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        <p className="text-xs text-muted">
          Writes <span className="text-fg">{DEMO_SERIES}</span> via <code>ingest.write</code>, then
          reads it back via <code>series.latest</code> — write → stage → drain → read, through the
          bridge.
        </p>

        {writer.state.status === "error" && (
          <p className="text-accent">Could not write: {writer.state.error}</p>
        )}

        <div className="rounded-md border border-border p-3">
          <p className="mb-1 text-xs text-muted">
            Latest committed · <span className="text-fg">{DEMO_SERIES}</span>
          </p>
          {latest.state.status === "loading" && <p>Reading latest…</p>}
          {latest.state.status === "error" && (
            <p className="text-accent">Could not read latest: {latest.state.error}</p>
          )}
          {latest.state.status === "ready" && latest.state.data === null && (
            <p>No samples committed yet — click Write sample.</p>
          )}
          {latest.state.status === "ready" && latest.state.data !== null && (
            <p className="text-fg" data-testid="demo-latest">
              seq {String(latest.state.data.seq)} · value {describe(latest.state.data.payload)}
            </p>
          )}
        </div>

        {lastWritten !== null && (
          <p className="text-xs text-muted" data-testid="demo-last-written">
            Last written value: {lastWritten}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

/** Render an arbitrary JSON payload as a readable string (the platform carries heterogeneous series). */
function describe(payload: unknown): string {
  if (payload === undefined) return "(no payload)";
  if (typeof payload === "object") return JSON.stringify(payload);
  return String(payload);
}
