// LogPanel — render a run's `log(...)` lines (rules-workbench scope). One render component per concern.

import type { LogLine } from "@/lib/rules";

interface LogPanelProps {
  log: LogLine[];
}

export function LogPanel({ log }: LogPanelProps) {
  if (log.length === 0) return null;
  return (
    <div aria-label="log panel" className="rounded-md border border-border bg-muted p-3">
      <div className="text-xs uppercase tracking-wide text-muted">Log</div>
      <ul className="mt-1 space-y-0.5 font-mono text-xs text-fg">
        {log.map((l, i) => (
          <li key={i} aria-label="log line">
            <span className="text-muted">[{l.level}]</span> {l.message}
          </li>
        ))}
      </ul>
    </div>
  );
}
