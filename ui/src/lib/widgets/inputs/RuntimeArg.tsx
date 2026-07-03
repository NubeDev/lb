// The runtime arg widget (external-agent run-lifecycle #5). An `x-lb-widget:"runtime"` arg renders
// this: a dropdown whose FIRST entry is the workspace's ACTIVE pick and every other entry is a
// concrete runtime id (an explicit per-message override). Mirrors `SqlArg`'s shape (the data hook is
// passed-in-by-import; this file is RENDER + local selection only, FILE-LAYOUT).
//
// Value model: the EMPTY string means "active pick — send NO `runtime`" so the host's fallback runs
// (explicit → agent.config.default_runtime → registry default). The Active entry maps to "" and is
// selected on mount, so a composer that never touches the dropdown defeats nothing — it sends no
// runtime and the workspace's active pick wins. A concrete id is an EXPLICIT override (sent verbatim).

import { Select } from "@/components/ui/select";
import { useRuntimes } from "./useRuntimes";

interface Props {
  /** The chosen runtime id — empty means "active pick, runtime omitted"; a concrete id is an override. */
  value: string;
  onChange: (runtime: string) => void;
}

export function RuntimeArg({ value, onChange }: Props) {
  const { runtimes, defaultId, workspaceDefault, loading, error } = useRuntimes();

  // The Active entry's label: the workspace pick's human label when one is set, else the registry
  // default id (the effective active when the workspace has picked nothing).
  const activeLabel = workspaceDefault
    ? `Active — ${workspaceDefault.label}`
    : `Active — ${defaultId} (in-house)`;

  // The concrete-override options: every configured id (or just the default until the fetch resolves).
  const overrides = runtimes.length > 0 ? runtimes : [defaultId];

  return (
    <div className="border-t border-border bg-panel p-2" aria-label="runtime picker">
      <label className="mb-1 block text-xs text-muted" htmlFor="runtime-arg">
        Runtime
      </label>
      <Select
        id="runtime-arg"
        aria-label="runtime"
        value={value}
        disabled={loading}
        onChange={(e) => onChange(e.target.value)}
      >
        {/* The ACTIVE pick — maps to "" (no runtime on the wire, the host resolves the active pick). */}
        <option value="">{activeLabel}</option>
        {/* Explicit per-message overrides: the concrete runtime ids. */}
        {overrides.map((id) => (
          <option key={id} value={id}>
            {id}
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
