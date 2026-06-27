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
    <div className="border-b border-border px-3 py-2">
      <div className="mb-1 flex items-center gap-1 text-xs font-medium text-muted">
        <Building2 size={12} /> Workspace
      </div>
      {error && (
        <div role="alert" className="mb-1 text-xs text-accent">
          {error}
        </div>
      )}
      <select
        aria-label="workspace"
        className="w-full rounded bg-panel px-2 py-1 text-sm"
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
        className="mt-1 flex gap-1"
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
          className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-xs"
          placeholder="new workspace…"
          value={newWs}
          onChange={(e) => setNewWs(e.target.value)}
        />
        <button aria-label="create workspace" className="rounded bg-accent/15 px-2 text-accent">
          <Plus size={14} />
        </button>
      </form>
    </div>
  );
}
