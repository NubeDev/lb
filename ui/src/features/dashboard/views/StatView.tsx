// The v2 `stat` view — the latest single value of a source, large. Reads `source` through the bridge
// (`useSource`); a non-numeric latest renders verbatim. A denied/empty source shows an honest state,
// never a fake number. The v2 generalization of the v1 StatWidget (source replaces series binding).

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { useSource } from "../builder/useSource";
import { asNumber } from "./num";
import type { Source } from "@/lib/dashboard";

interface Props {
  source?: Source;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function StatView({ source, tools, options, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { latest, loading, denied } = useSource(source, tools, scope, refreshKey);
  const unit = typeof options?.unit === "string" ? (options.unit as string) : "";

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;
  if (latest == null) return <WidgetMessage tone="muted">no value yet</WidgetMessage>;

  const n = asNumber(latest);
  const display = n !== null ? n : String(latest);

  return (
    <div className="flex h-full flex-col" aria-label={`stat ${source?.tool ?? ""}`}>
      <WidgetHeader label={label ?? source?.tool ?? ""} />
      <div className="flex flex-1 items-center justify-center">
        <span className="text-3xl font-semibold text-fg" aria-label="stat value">
          {display}
          {unit && <span className="ml-1 text-base text-muted">{unit}</span>}
        </span>
      </div>
    </div>
  );
}
