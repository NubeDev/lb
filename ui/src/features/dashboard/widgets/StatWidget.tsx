// The stat / single-value widget — the latest value of a series, large (dashboard scope). Reads the
// newest sample via `useSeries` (backfill + live SSE); a non-numeric payload (e.g. a state string)
// renders verbatim. An un-granted binding shows an honest denied state, never a fake number.

import { useSeries } from "../useSeries";
import { asNumber } from "./num";
import { WidgetHeader, WidgetMessage } from "./chrome";
import type { Binding } from "@/lib/dashboard";
import type { DashboardSearch } from "@/features/routing/search";

interface Props {
  binding: Binding;
  options?: Record<string, unknown>;
  range?: DashboardSearch;
  label?: string;
}

export function StatWidget({ binding, options, range, label }: Props) {
  const { series, latest, loading, denied } = useSeries(binding, range);
  const unit = typeof options?.unit === "string" ? (options.unit as string) : "";

  if (denied) return <WidgetMessage tone="denied">no access to this series</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;
  if (latest === null) return <WidgetMessage tone="muted">no samples yet</WidgetMessage>;

  const n = asNumber(latest.payload);
  const display = n !== null ? n : String(latest.payload);

  return (
    <div className="flex h-full flex-col" aria-label={`stat ${series ?? ""}`}>
      <WidgetHeader label={label ?? series ?? ""} />
      <div className="flex flex-1 items-center justify-center">
        <span className="text-3xl font-semibold text-fg" aria-label="stat value">
          {display}
          {unit && <span className="ml-1 text-base text-muted">{unit}</span>}
        </span>
      </div>
    </div>
  );
}
