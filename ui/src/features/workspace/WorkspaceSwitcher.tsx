// The workspace switcher — shows the current workspace and the workspaces this identity belongs to
// (resolved through `identity.workspaces`, global-identity scope), and switches by re-logging in to
// the chosen workspace (the workspace is the token's hard wall §7, so switching is a re-login, not a
// client-side toggle). Markup + wiring only; data lives in useMyWorkspaces (the identity's
// memberships) + useWorkspaces (the directory). On shadcn primitives (Select/Alert) + tokens.

import { Building2 } from "lucide-react";

import { Alert } from "@/components/ui/alert";
import { Select } from "@/components/ui/select";
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
  const { workspaces, error } = useWorkspaces();
  const { mine } = useMyWorkspaces(principal);

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
        <Alert variant="destructive" className="mb-2 px-2 py-1.5 text-xs">
          {error}
        </Alert>
      )}
      <Select
        aria-label="workspace"
        className="h-8 w-full"
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
      </Select>
    </div>
  );
}
