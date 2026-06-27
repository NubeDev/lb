// The workspace switcher — shows the current workspace and the directory, and switches by
// re-logging in to the chosen workspace (the workspace is the token's hard wall §7, so switching is
// a re-login, not a client-side toggle). Markup + wiring only; data lives in useWorkspaces.

import { useState } from "react";
import { Building2, Plus } from "lucide-react";

import { useWorkspaces } from "./useWorkspaces";

interface Props {
  /** The current workspace (from the session). */
  current: string;
  /** Switch to `ws` — the app re-logs in to make it the token's workspace. */
  onSwitch: (ws: string) => void;
}

export function WorkspaceSwitcher({ current, onSwitch }: Props) {
  const { workspaces, error, create } = useWorkspaces();
  const [newWs, setNewWs] = useState("");

  return (
    <div className="border-b border-border px-3 py-3">
      <div className="mb-2 flex items-center gap-1.5 text-xs font-semibold text-muted">
        <Building2 size={13} /> Workspace
      </div>
      {error && (
        <div role="alert" className="mb-2 rounded-md border border-red-500/25 bg-red-500/10 px-2 py-1.5 text-xs text-red-600 dark:text-red-300">
          {error}
        </div>
      )}
      <select
        aria-label="workspace"
        className="control-field w-full"
        value={current}
        onChange={(e) => onSwitch(e.target.value)}
      >
        {/* The current workspace is always selectable even if the directory list hasn't loaded it. */}
        {!workspaces.some((w) => w.ws === current) && <option value={current}>{current}</option>}
        {workspaces.map((w) => (
          <option key={w.ws} value={w.ws}>
            {w.name || w.ws}
          </option>
        ))}
      </select>
      <form
        className="mt-2 flex gap-1.5"
        onSubmit={(e) => {
          e.preventDefault();
          const ws = newWs.trim();
          if (ws) {
            void create(ws, ws);
            setNewWs("");
          }
        }}
      >
        <input
          aria-label="new workspace"
          className="control-field-sm min-w-0 flex-1"
          placeholder="new workspace…"
          value={newWs}
          onChange={(e) => setNewWs(e.target.value)}
        />
        <button aria-label="create workspace" className="soft-button-sm px-2">
          <Plus size={14} />
        </button>
      </form>
    </div>
  );
}
