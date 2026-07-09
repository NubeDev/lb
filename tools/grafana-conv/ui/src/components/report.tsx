// Renders the `ConversionReport` — the headline honesty deliverable. Grouped by
// fate (mapped / degraded / dropped) exactly like the audit matrix the user
// reasons about.

import type { ConversionReport, Fate, ReportLine } from "@/types";
import { cn } from "@/lib/cn";

const GROUPS: { fate: Fate; label: string; color: string }[] = [
  { fate: "mapped", label: "Mapped", color: "text-emerald-400" },
  { fate: "degraded", label: "Degraded", color: "text-amber-400" },
  { fate: "dropped", label: "Dropped", color: "text-rose-400" },
];

export function Report({ report }: { report: ConversionReport }) {
  const groups = GROUPS.map((g) => ({ ...g, lines: report[g.fate] }));
  const total = groups.reduce((n, g) => n + g.lines.length, 0);

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-4 text-xs">
        <span className="text-neutral-400">{total} lines</span>
        {groups.map((g) => (
          <span key={g.fate} className={cn("font-medium", g.color)}>
            {g.label}: {g.lines.length}
          </span>
        ))}
      </div>
      <div className="space-y-4">
        {groups.map((g) =>
          g.lines.length === 0 ? null : (
            <section key={g.fate}>
              <h3 className={cn("text-xs font-semibold uppercase tracking-wide mb-2", g.color)}>
                {g.label}
              </h3>
              <ul className="space-y-1">
                {g.lines.map((l, i) => <Line key={i} line={l} />)}
              </ul>
            </section>
          ),
        )}
      </div>
    </div>
  );
}

function Line({ line }: { line: ReportLine }) {
  return (
    <li className="text-xs leading-relaxed">
      <code className="text-neutral-300 bg-neutral-800 rounded px-1 py-0.5">{line.code}</code>
      {line.at && <span className="text-neutral-500"> at {line.at}</span>}
      <span className="text-neutral-400"> — {line.reason}</span>
    </li>
  );
}
