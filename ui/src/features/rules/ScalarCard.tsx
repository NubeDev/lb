// ScalarCard — render a `RuleOutput` of kind "scalar": the single value a rule's last expression
// resolved to (rules-workbench scope). A true scalar (number / string / bool) shows as the big headline
// value; a structured scalar (an object/array — e.g. a `channel.history` array of items) renders as the
// interactive, deep-parsed JSON tree instead of an escaped one-line `JSON.stringify` blob (the tree is
// the shared `JsonTree`, so nested JSON-string fields like a channel row's `body` expand humanely). One
// render component per output kind (FILE-LAYOUT).

import { JsonTree } from "./JsonTree";

interface ScalarCardProps {
  value: unknown;
}

export function ScalarCard({ value }: ScalarCardProps) {
  // A structured value (array/object) is not a "scalar" a headline can show — render the tree.
  if (value !== null && typeof value === "object") {
    return (
      <div aria-label="scalar result" className="rounded-md border border-border p-1">
        <div className="px-3 pt-2 text-xs uppercase tracking-wide text-muted">Result</div>
        <div className="p-2">
          <JsonTree src={value} />
        </div>
      </div>
    );
  }

  const text = typeof value === "string" ? value : JSON.stringify(value);
  return (
    <div aria-label="scalar result" className="rounded-md border border-border p-4">
      <div className="text-xs uppercase tracking-wide text-muted">Result</div>
      <div aria-label="scalar value" className="mt-1 font-mono text-2xl">
        {text}
      </div>
    </div>
  );
}
