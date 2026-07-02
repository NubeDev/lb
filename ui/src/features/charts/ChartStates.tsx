// The non-data states a chart can be in — loading, denied, empty, and "table-only" (data exists but no
// numeric column to plot). Shared by both surfaces so an empty dashboard panel and an empty channel
// chart teach the same thing instead of each inventing a blank box. Product-register rule: empty states
// teach the interface, not "nothing here".
//
// One responsibility: render a centered chart placeholder with an icon + line.

import { BarChart3, Lock, Loader2, TableProperties } from "lucide-react";

type Tone = "loading" | "denied" | "empty" | "table-only";

const CONTENT: Record<Tone, { icon: typeof BarChart3; title: string; hint?: string }> = {
  loading: { icon: Loader2, title: "Loading…" },
  denied: { icon: Lock, title: "No access to this source" },
  empty: { icon: BarChart3, title: "No data yet", hint: "This chart draws as soon as the query returns rows." },
  "table-only": {
    icon: TableProperties,
    title: "Nothing numeric to plot",
    hint: "Pick a numeric field for the y axis, or view the result as a table.",
  },
};

export function ChartState({ tone }: { tone: Tone }) {
  const { icon: Icon, title, hint } = CONTENT[tone];
  return (
    <div
      className="flex h-full min-h-24 w-full flex-col items-center justify-center gap-2 px-4 text-center"
      role="status"
    >
      <Icon
        size={20}
        className={tone === "loading" ? "animate-spin text-muted" : "text-muted/70"}
        aria-hidden
      />
      <p className="text-sm font-medium text-fg/80">{title}</p>
      {hint && <p className="max-w-[40ch] text-xs text-muted">{hint}</p>}
    </div>
  );
}
