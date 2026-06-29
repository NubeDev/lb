// FindingsList — render a run's findings (emit/alert): level-coloured, alert-marked (rules-workbench
// scope). Shown when the output kind is "findings", or alongside any run that emitted findings. One
// render component per concern (FILE-LAYOUT).

import type { Finding } from "@/lib/rules";

interface FindingsListProps {
  findings: Finding[];
}

/** Map a finding level to a Tailwind colour class — critical/warning/info, with a sane default. */
function levelClass(level: string): string {
  switch (level) {
    case "critical":
      return "border-red-300 bg-red-50 text-red-800";
    case "warning":
      return "border-amber-300 bg-amber-50 text-amber-800";
    default:
      return "border-sky-300 bg-sky-50 text-sky-800";
  }
}

export function FindingsList({ findings }: FindingsListProps) {
  if (findings.length === 0) return null;
  return (
    <ul aria-label="findings" className="space-y-2">
      {findings.map((f, i) => (
        <li
          key={i}
          aria-label={`finding ${f.level}`}
          className={`rounded border px-3 py-2 text-sm ${levelClass(f.level)}`}
        >
          <span className="font-semibold uppercase">{f.level}</span>
          {f.data.alert ? (
            <span aria-label="alert mark" className="ml-2 rounded bg-red-600 px-1 text-xs text-white">
              ALERT
            </span>
          ) : null}
          <span className="ml-2 font-mono">{JSON.stringify(f.data)}</span>
        </li>
      ))}
    </ul>
  );
}
