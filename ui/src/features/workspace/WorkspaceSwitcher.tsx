// The workspace switcher — shows the current workspace and the workspaces this identity belongs to
// (resolved through `identity.workspaces`, global-identity scope), and switches by re-logging in to
// the chosen workspace (the workspace is the token's hard wall §7, so switching is a re-login, not a
// client-side toggle). Markup + wiring only; data lives in useWorkspaces (the directory) +
// useMyWorkspaces (the identity's memberships).

import { useState } from "react";
import { Building2, Plus } from "lucide-react";

import { useMyWorkspaces } from "./useMyWorkspaces";
import { useWorkspaces } from "./useWorkspaces";

interface Props {
  /** The current workspace (from the session). */
  current: string;
  /** The logged-in principal (`user:…`) — drives `identity.workspaces`. */
  principal?: string;
  /** Switch to `ws` — the app re-logs in to make it the token's workspace. */
  onSwitch: (ws: string) => void;
}

export function WorkspaceSwitcher({ current, principal, onSwitch }: Props) {
  const { workspaces, error, create } = useWorkspaces();
  const { mine } = useMyWorkspaces(principal);
  const [newWs, setNewWs] = useState("");

  // Prefer the identity's memberships; fall back to the node directory when the identity has not
  // resolved any (e.g. a fresh dev login before the directory lists it).
  const directoryIds = new Set(workspaces.map((w) => w.ws));
  const extraMine = mine.filter((m) => !directoryIds.has(m.ws));
  const known = mine.length > 0 ? mine.map((m) => ({ ws: m.ws, name: m.name || m.ws })) : workspaces;

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
        {/* The current workspace is always selectable even if the list hasn't loaded it. */}
        {!known.some((w) => w.ws === current) && extraMine.every((m) => m.ws !== current) && (
          <option value={current}>{current}</option>
        )}
        {known.map((w) => (
          <option key={w.ws} value={w.ws}>
            {w.name || w.ws}
          </option>
        ))}
        {extraMine
          .filter((m) => !known.some((w) => w.ws === m.ws))
          .map((m) => (
            <option key={m.ws} value={m.ws}>
              {m.name || m.ws}
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
