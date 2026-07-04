// JsonView — the interactive JSON view of a run result (rules-editor-ux scope). The workbench's result
// region toggles between the typed views (table / scalar / findings) and this: the whole `RunResult` as
// a collapsible, syntax-highlighted tree (the shared `JsonTree`, which deep-parses embedded JSON-string
// fields so a nested channel payload expands humanely). Faithful, never abridged — the product principle
// "make raw data humane, keep values faithful". One component per file (FILE-LAYOUT).

import type { RunResult } from "@/lib/rules";
import { JsonTree } from "./JsonTree";

interface JsonViewProps {
  result: RunResult;
}

export function JsonView({ result }: JsonViewProps) {
  return (
    <div
      aria-label="json result"
      className="overflow-auto rounded-md border border-border bg-bg p-3 text-[12.5px]"
    >
      <JsonTree src={result} />
    </div>
  );
}
