// The raw-JSON options escape hatch (editor-parity scope, step 3) — ONLY for an imported transform id
// with no typed editor (never a shipped one). Writes back only when the text parses to an object, so
// invalid JSON never corrupts `state.transformations`. One responsibility: edit an opaque options bag.

import { Textarea } from "@/components/ui/textarea";

export function RawJsonOptions({
  opts,
  onChange,
}: {
  opts: Record<string, unknown>;
  onChange: (o: Record<string, unknown>) => void;
}) {
  return (
    <label className="grid gap-1 text-xs text-muted">
      Options (JSON)
      <Textarea
        aria-label="transform options json"
        className="h-16 w-full resize-y py-1 font-mono text-xs"
        defaultValue={JSON.stringify(opts, null, 0)}
        onBlur={(e) => {
          try {
            const parsed = JSON.parse(e.target.value || "{}");
            if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) onChange(parsed as Record<string, unknown>);
          } catch {
            /* invalid JSON: keep the prior config, don't corrupt state */
          }
        }}
      />
    </label>
  );
}
