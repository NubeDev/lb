// The query status bar under the Data Studio preview (data-studio-ux scope, "make the loop honest"). It
// turns the panel-data hook's state into Grafana-Explore-grade feedback: is a query running, did it
// error (show the text, not a silent empty chart), how many rows/frames came back, how long it took, and
// — the common confusion — WHY there's no data (never ran vs. ran-and-empty). It is a pure view over the
// `SourceState` the preview already reads; no new fetch, no new state store.
//
// One responsibility: render one line of query status. The provenance chip ("shaped from cached data")
// is the visible payoff of the fetch/shape split — an option edit reshapes without re-querying.

import { AlertTriangle, CheckCircle2, CircleDashed, Database, Loader2, Snowflake } from "lucide-react";

import type { SourceState } from "@/features/dashboard/builder/useSource";

interface Props {
  state: SourceState;
  /** Whether the draft has a resolvable primary target at all — distinguishes "never ran" from "ran
   *  and returned nothing". */
  hasTarget: boolean;
  /** The current time-range label (e.g. "last 6h" / "04/06–04/07"), shown on an empty result so the user
   *  knows what window returned zero rows. Optional. */
  rangeLabel?: string;
  /** Whether the preview is frozen (edit-without-requery). Shown as a chip so it's never a mystery why a
   *  source edit didn't re-fetch. */
  frozen?: boolean;
}

/** Format a duration in ms compactly. */
function ms(n: number | undefined): string | null {
  if (n === undefined) return null;
  if (n < 1) return "<1 ms";
  if (n < 1000) return `${Math.round(n)} ms`;
  return `${(n / 1000).toFixed(2)} s`;
}

/** "as of HH:MM:SS" from an epoch-ms fetch time (local). */
function asOf(at: number | undefined): string | null {
  if (!at) return null;
  const d = new Date(at);
  const hh = String(d.getHours()).padStart(2, "0");
  const mm = String(d.getMinutes()).padStart(2, "0");
  const ss = String(d.getSeconds()).padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}

export function QueryStatusBar({ state, hasTarget, rangeLabel, frozen }: Props) {
  const { rows, loading, denied, meta } = state;
  const duration = ms(meta?.ms);
  const at = asOf(meta?.fetchedAt);

  let icon = <CircleDashed size={12} className="text-muted" aria-hidden />;
  let text: React.ReactNode = "Pick a source to run a query";
  let tone = "text-muted";

  if (loading) {
    icon = <Loader2 size={12} className="animate-spin text-accent" aria-hidden />;
    text = "Running query…";
    tone = "text-fg";
  } else if (!hasTarget) {
    // Never ran — say what's missing, not "no data yet".
    icon = <CircleDashed size={12} className="text-muted" aria-hidden />;
    text = "No source selected — pick one in the Query tab to see data";
    tone = "text-muted";
  } else if (denied) {
    // Ran and failed — the error text inline (a deny reads as "Denied"), never a blank chart.
    icon = <AlertTriangle size={12} className="text-danger" aria-hidden />;
    text = meta?.error ? `Query error — ${meta.error}` : "Query denied";
    tone = "text-danger";
  } else if (rows.length === 0) {
    // Ran fine, zero rows — distinguish from "never ran" and name the window.
    icon = <Database size={12} className="text-warn" aria-hidden />;
    text = rangeLabel
      ? `Query returned 0 rows for ${rangeLabel}`
      : "Query returned 0 rows";
    tone = "text-warn";
  } else {
    icon = <CheckCircle2 size={12} className="text-ok" aria-hidden />;
    const parts = [`${rows.length.toLocaleString()} row${rows.length === 1 ? "" : "s"}`];
    if (meta?.frames && meta.frames > 1) parts.push(`${meta.frames} frames`);
    if (duration) parts.push(duration);
    if (at) parts.push(`as of ${at}`);
    text = parts.join(" · ");
    tone = "text-fg";
  }

  return (
    <div
      role="status"
      aria-label="query status"
      className="flex items-center gap-2 rounded-md border border-border bg-panel/60 px-2 py-1 text-[11px]"
    >
      {icon}
      <span className={tone}>{text}</span>
      <span className="flex-1" />
      {frozen && (
        <span className="flex items-center gap-1 rounded-md bg-accent/10 px-1.5 py-0.5 text-[10px] text-accent" title="Preview is frozen — edits reshape the data already fetched, without re-querying">
          <Snowflake size={10} aria-hidden /> frozen
        </span>
      )}
      {!denied && !loading && meta?.source === "shaped" && (
        <span
          className="rounded-md bg-ok/10 px-1.5 py-0.5 text-[10px] text-ok"
          title="Reshaped from data already fetched — the datasource was not re-queried"
        >
          shaped from cached data
        </span>
      )}
      {!denied && !loading && meta?.source === "live" && (
        <span className="rounded-md bg-accent/10 px-1.5 py-0.5 text-[10px] text-accent">live</span>
      )}
    </div>
  );
}
