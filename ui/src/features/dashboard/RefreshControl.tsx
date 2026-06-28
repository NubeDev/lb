// The auto-refresh picker (widget-config-vars Slice 4). A dropdown (off/5s/10s/30s/1m/5m/15m), URL-synced
// `?refresh=30s`, that on each tick bumps a `refreshKey` re-resolving query variables + re-running each
// cell's read source (the proof-panel refreshKey pattern, generalized). Refresh polls STATE; a cell's
// `bus.watch`/`series.watch` streams MOTION — they compose. The interval lives in the URL (shareable);
// the tick is owned by `useAutoRefresh`. Pauses when the tab is hidden; debounced; in-flight dedupe is
// the source hooks' job (a re-keyed effect cancels the prior run).

import { REFRESH_OPTIONS } from "@/features/routing/search";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

const LABEL: Record<string, string> = {
  "": "off",
  "5s": "5s",
  "10s": "10s",
  "30s": "30s",
  "1m": "1m",
  "5m": "5m",
  "15m": "15m",
};

interface Props {
  /** The current interval (from the URL; `""`/undefined = off). */
  value: string | undefined;
  /** Write the chosen interval to the URL (the parent maps to a router navigate). */
  onChange: (refresh: string | undefined) => void;
}

/** The refresh-interval dropdown. Selecting `off` clears the URL param. */
export function RefreshControl({ value, onChange }: Props) {
  return (
    <label className="flex items-center gap-1.5 text-xs text-muted" title="Auto-refresh interval">
      <span>refresh</span>
      {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive; token-bound native */}
      <select
        aria-label="refresh interval"
        className={FIELD}
        value={value ?? ""}
        onChange={(e) => onChange(e.target.value || undefined)}
      >
        {REFRESH_OPTIONS.map((o) => (
          <option key={o} value={o}>
            {LABEL[o] ?? o}
          </option>
        ))}
      </select>
    </label>
  );
}
