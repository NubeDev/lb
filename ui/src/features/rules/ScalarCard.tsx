// ScalarCard — render a `RuleOutput` of kind "scalar": the single value a rule's last expression
// resolved to (rules-workbench scope). One render component per output kind (FILE-LAYOUT).

interface ScalarCardProps {
  value: unknown;
}

export function ScalarCard({ value }: ScalarCardProps) {
  const text = typeof value === "string" ? value : JSON.stringify(value);
  return (
    <div aria-label="scalar result" className="rounded border border-border p-4">
      <div className="text-xs uppercase tracking-wide text-muted">Result</div>
      <div aria-label="scalar value" className="mt-1 font-mono text-2xl">
        {text}
      </div>
    </div>
  );
}
