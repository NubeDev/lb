import { BellRing } from "lucide-react";

import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useSeriesFind } from "@/data/useSeriesFind";
import { useSeriesLatest } from "@/data/useSeriesLatest";

/** Nested child: probes the bridge by reading the latest sample of the first known series. It reports
 *  the bridge connection state HONESTLY — no fabricated alerts. Empty/denied → an honest Card. */
export function Alerts() {
  const find = useSeriesFind([]);
  const first =
    find.state.status === "ready" && find.state.data.length > 0
      ? (find.state.data[0].name ?? find.state.data[0].id ?? null)
      : null;
  const latest = useSeriesLatest(first);

  return (
    <Card>
      <CardHeader>
        <CardTitle>
          <BellRing className="h-4 w-4 text-accent" aria-hidden />
          Alerts
        </CardTitle>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {find.state.status === "loading" && <p>Connecting to the bridge…</p>}
        {find.state.status === "error" && (
          <p className="text-accent">Bridge error: {find.state.error}</p>
        )}
        {find.state.status === "ready" && !first && (
          <p>No series to monitor — no alerts. The bridge is reachable but returned no data.</p>
        )}
        {first && latest.state.status === "loading" && <p>Reading latest sample for “{first}”…</p>}
        {first && latest.state.status === "error" && (
          <p className="text-accent">Could not read latest for “{first}”: {latest.state.error}</p>
        )}
        {first && latest.state.status === "ready" && (
          <p>
            Bridge connected. Latest sample for <span className="text-fg">{first}</span>:{" "}
            <span className="text-fg">{describe(latest.state.data)}</span>. No alert rules configured
            this slice.
          </p>
        )}
      </CardContent>
    </Card>
  );
}

function describe(sample: { value?: number | string } | null): string {
  if (!sample) return "no sample";
  return sample.value === undefined ? "received (no value field)" : String(sample.value);
}
