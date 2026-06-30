// The telemetry console filter bar (telemetry-console scope): source / actor / level / outcome /
// trace_id / free-text + a live-tail toggle. One component; presentation + the controlled inputs that
// drive `useTelemetry`'s filter. The filters are URL-encoded by the view (shareable), so this only
// reads/writes the filter object.

import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import type { TelemetryFilter, TelemetryLevel, TelemetryOutcome } from "@/lib/telemetry";

const LEVELS: TelemetryLevel[] = ["error", "warn", "info", "debug", "trace"];
const OUTCOMES: TelemetryOutcome[] = ["allow", "deny", "error"];

interface Props {
  filter: TelemetryFilter;
  onChange: (next: TelemetryFilter) => void;
  live: boolean;
  onLive: (on: boolean) => void;
}

/** The composable filter bar. Every control patches one field; an empty value clears that clause. */
export function TelemetryFilterBar({ filter, onChange, live, onLive }: Props) {
  const patch = (p: Partial<TelemetryFilter>) => onChange({ ...filter, ...p });

  return (
    <div className="flex flex-wrap items-center gap-2 border-b border-border pb-3">
      <Input
        aria-label="source"
        placeholder="source (e.g. mqtt)"
        className="w-40"
        value={filter.source ?? ""}
        onChange={(e) => patch({ source: e.target.value || undefined })}
      />
      <Input
        aria-label="actor"
        placeholder="actor (e.g. user:ada)"
        className="w-44"
        value={filter.actor ?? ""}
        onChange={(e) => patch({ actor: e.target.value || undefined })}
      />
      <Select
        aria-label="level"
        className="w-32"
        value={filter.level ?? ""}
        onChange={(e) => patch({ level: (e.target.value || undefined) as TelemetryLevel | undefined })}
      >
        <option value="">level ≥ any</option>
        {LEVELS.map((l) => (
          <option key={l} value={l}>
            ≥ {l}
          </option>
        ))}
      </Select>
      <Select
        aria-label="outcome"
        className="w-32"
        value={filter.outcome ?? ""}
        onChange={(e) =>
          patch({ outcome: (e.target.value || undefined) as TelemetryOutcome | undefined })
        }
      >
        <option value="">any outcome</option>
        {OUTCOMES.map((o) => (
          <option key={o} value={o}>
            {o}
          </option>
        ))}
      </Select>
      <Input
        aria-label="trace_id"
        placeholder="trace_id"
        className="w-44"
        value={filter.traceId ?? ""}
        onChange={(e) => patch({ traceId: e.target.value || undefined })}
      />
      <Input
        aria-label="text"
        placeholder="search message…"
        className="w-48 flex-1"
        value={filter.text ?? ""}
        onChange={(e) => patch({ text: e.target.value || undefined })}
      />
      <label className="flex select-none items-center gap-1.5 text-sm text-muted-foreground">
        {/* eslint-disable-next-line no-restricted-syntax -- a checkbox has no shadcn primitive in this repo */}
        <input
          type="checkbox"
          aria-label="live tail"
          checked={live}
          onChange={(e) => onLive(e.target.checked)}
        />
        Live tail
      </label>
    </div>
  );
}
