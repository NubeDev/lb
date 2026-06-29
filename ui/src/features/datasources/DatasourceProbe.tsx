// The connectivity-probe badge (rules-workbench scope, Phase 3) — a green/red indicator for a
// `datasource.test` result. GREEN on `ok`; RED with the error message on a non-ok (a real sidecar
// fault / refused endpoint / missing source). It never shows a fake green — an unprobed source shows a
// neutral "Test" affordance, a red probe shows the honest error. One responsibility, one file.

import { Button } from "@/components/ui/button";
import type { ProbeResult } from "@/lib/datasources";

interface Props {
  name: string;
  result: ProbeResult | undefined;
  onTest: (name: string) => void;
}

export function DatasourceProbe({ name, result, onTest }: Props) {
  if (result === undefined) {
    return (
      <Button
        aria-label={`test ${name}`}
        size="sm"
        variant="ghost"
        onClick={() => onTest(name)}
      >
        Test
      </Button>
    );
  }
  if (result.ok) {
    return (
      <span
        aria-label={`probe ${name}`}
        data-state="green"
        className="rounded bg-green-500/15 px-2 py-0.5 text-xs text-green-400"
      >
        Connected
      </span>
    );
  }
  return (
    <span
      aria-label={`probe ${name}`}
      data-state="red"
      title={result.error}
      className="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
    >
      Failed{result.error ? `: ${result.error}` : ""}
    </span>
  );
}
