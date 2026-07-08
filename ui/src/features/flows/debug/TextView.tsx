// TextView — monospace, whitespace-preserved text for the debug panel (debug-node-scope). The plain
// `format:"text"` renderer: numbers, booleans, plain strings, null. One responsibility: a text value
// in, a `<pre>` out. Long-content collapse lives on the row (`DebugMessageRow`), not here.

interface Props {
  /** The value rendered as text (coerced to string). */
  value: unknown;
}

export function TextView({ value }: Props) {
  const text = typeof value === "string" ? value : value === null ? "null" : JSON.stringify(value);
  return (
    <pre className="overflow-x-auto rounded-md border border-border bg-panel-2/40 p-2 font-mono text-xs leading-5 text-fg/90">
      {text}
    </pre>
  );
}
