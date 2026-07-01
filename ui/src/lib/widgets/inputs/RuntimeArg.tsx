// The runtime arg widget (external-agent run-lifecycle #5). An `x-lb-widget:"runtime"` arg renders
// this: a dropdown of the node's configured agent runtimes (`agent.runtimes` via `useRuntimes`),
// with the default id preselected — replacing the old typed `@id`. Mirrors `SqlArg`'s shape (the
// data hook is passed-in-by-import; this file is RENDER + local selection only, FILE-LAYOUT).
//
// The empty parent `value` means "unset" → the widget shows the default id; picking any option calls
// `onChange` with the chosen id. The agent command's `runtime` is optional (absent → in-house
// default at the host), so a value equal to the default id may be sent verbatim or omitted upstream —
// the host resolves both identically (absent → default).

import { useEffect } from "react";

import { Select } from "@/components/ui/select";
import { useRuntimes } from "./useRuntimes";

interface Props {
  /** The chosen runtime id (empty until the user picks / the default lands). */
  value: string;
  onChange: (runtime: string) => void;
}

export function RuntimeArg({ value, onChange }: Props) {
  const { runtimes, defaultId, loading, error } = useRuntimes();

  // Preselect the default once it loads (only if the parent hasn't already set a value).
  useEffect(() => {
    if (!value && defaultId) onChange(defaultId);
  }, [value, defaultId, onChange]);

  // The options: the fetched ids, or (until the fetch resolves / on error) just the default so the
  // picker is never empty and the command stays runnable.
  const options = runtimes.length > 0 ? runtimes : [defaultId];
  const selected = value || defaultId;

  return (
    <div className="border-t border-border bg-panel p-2" aria-label="runtime picker">
      <label className="mb-1 block text-xs text-muted" htmlFor="runtime-arg">
        Runtime
      </label>
      <Select
        id="runtime-arg"
        aria-label="runtime"
        value={selected}
        disabled={loading}
        onChange={(e) => onChange(e.target.value)}
      >
        {options.map((id) => (
          <option key={id} value={id}>
            {id === defaultId ? `${id} (in-house)` : id}
          </option>
        ))}
      </Select>
      {error && (
        <div role="alert" className="mt-1 text-xs text-destructive">
          {error}
        </div>
      )}
    </div>
  );
}
