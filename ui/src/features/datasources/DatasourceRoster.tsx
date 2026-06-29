// The datasource roster (rules-workbench scope, Phase 3) — the list of registered sources: name +
// kind + endpoint + a REDACTED secret ref. It NEVER renders a DSN (the response carries none). Each row
// offers a probe (green/red) and a remove. One responsibility, one file (FILE-LAYOUT).

import { Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { DatasourceSummary, ProbeResult } from "@/lib/datasources";
import { DatasourceProbe } from "./DatasourceProbe";

interface Props {
  sources: DatasourceSummary[];
  probes: Record<string, ProbeResult>;
  onTest: (name: string) => void;
  onRemove: (name: string) => void;
}

export function DatasourceRoster({ sources, probes, onTest, onRemove }: Props) {
  if (sources.length === 0) {
    return <p className="px-4 py-3 text-sm text-muted">No datasources yet.</p>;
  }
  return (
    <table className="w-full text-sm">
      <thead>
        <tr className="border-b border-border text-left text-xs text-muted">
          <th className="px-3 py-1.5 font-medium">Name</th>
          <th className="px-3 py-1.5 font-medium">Kind</th>
          <th className="px-3 py-1.5 font-medium">Endpoint</th>
          <th className="px-3 py-1.5 font-medium">Secret</th>
          <th className="px-3 py-1.5 font-medium">Probe</th>
          <th className="px-3 py-1.5" />
        </tr>
      </thead>
      <tbody>
        {sources.map((s) => (
          <tr key={s.name} aria-label={`datasource ${s.name}`} className="border-b border-border/50">
            <td className="px-3 py-1.5">{s.name}</td>
            <td className="px-3 py-1.5 text-muted">{s.kind}</td>
            <td className="px-3 py-1.5 font-mono text-xs text-muted">{s.endpoint}</td>
            {/* The redacted secret REF — a pointer, never the DSN value. */}
            <td className="px-3 py-1.5 font-mono text-xs text-muted" aria-label={`secret ref ${s.name}`}>
              {s.secretRef} <span className="text-muted/60">(redacted)</span>
            </td>
            <td className="px-3 py-1.5">
              <DatasourceProbe name={s.name} result={probes[s.name]} onTest={onTest} />
            </td>
            <td className="px-3 py-1.5 text-right">
              <Button
                aria-label={`remove ${s.name}`}
                size="sm"
                variant="destructive"
                onClick={() => onRemove(s.name)}
              >
                <Trash2 size={12} />
              </Button>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
