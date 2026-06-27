// Roles administration (admin-console redesign) — the REAL role editor the old UI never had. Left: a
// table of the workspace's roles with the count of caps each bundles. Right: the selected (or new)
// role's caps as a CHECKLIST, so you build a role by ticking capabilities instead of typing
// `role:<name>` strings. The candidate caps are the admin's OWN session caps (∪ caps already in any
// role) — which is exactly the no-widening set the server enforces, so the UI can't offer something
// the gateway will reject. Save calls `roles.define` (define replaces, so this is create AND edit).

import { useEffect, useState } from "react";
import { KeyRound } from "lucide-react";

import { AdminPanel } from "./AdminPanel";
import { useRoles } from "./useRoles";

interface Props {
  ws: string;
  /** The admin's session caps — the set they may bundle into a role (no-widening). */
  caps: string[] | undefined;
}

export function RolesAdmin({ ws, caps }: Props) {
  const { roles, error, define } = useRoles();
  const [selected, setSelected] = useState<string | null>(null);
  const [draftName, setDraftName] = useState("");
  const [draftCaps, setDraftCaps] = useState<Set<string>>(new Set());

  const selRole = roles.find((r) => r.name === selected) ?? null;

  // When the selection changes, seed the draft from that role (edit) or clear it (new).
  useEffect(() => {
    setDraftName(selRole?.name ?? "");
    setDraftCaps(new Set(selRole?.caps ?? []));
  }, [selRole]);

  // The caps the admin may bundle: their own session caps, plus any cap already used by a role so an
  // existing role stays editable even if it lists a cap the current admin lacks (it just can't be
  // re-added if removed — the server would reject widening).
  const candidates = Array.from(
    new Set([...(caps ?? []).filter((c) => c.startsWith("mcp:")), ...roles.flatMap((r) => r.caps)]),
  ).sort();

  function startNew() {
    setSelected(null);
    setDraftName("");
    setDraftCaps(new Set());
  }
  function toggle(cap: string) {
    setDraftCaps((prev) => {
      const next = new Set(prev);
      next.has(cap) ? next.delete(cap) : next.add(cap);
      return next;
    });
  }
  async function save() {
    const name = draftName.trim();
    if (!name) return;
    await define(name, [...draftCaps]);
    setSelected(name);
  }

  const action = (
    <button
      aria-label="new role"
      className="flex items-center gap-1 rounded bg-accent/15 px-2 py-1 text-xs text-accent"
      onClick={startNew}
    >
      <KeyRound size={13} /> New role
    </button>
  );

  return (
    <AdminPanel icon={KeyRound} title="Roles" ws={ws} action={action} error={error}>
      <div className="flex h-full">
        <div className="w-1/2 min-w-0 border-r border-border">
          {roles.length === 0 ? (
            <p className="px-4 py-3 text-sm text-muted">
              No roles yet. Create one to bundle capabilities.
            </p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs text-muted">
                  <th className="px-3 py-1.5 font-medium">Role</th>
                  <th className="px-3 py-1.5 font-medium">Capabilities</th>
                </tr>
              </thead>
              <tbody>
                {roles.map((r) => (
                  <tr
                    key={r.name}
                    aria-label={`select role ${r.name}`}
                    aria-selected={selected === r.name}
                    className={`cursor-pointer border-b border-border/50 ${
                      selected === r.name ? "bg-accent/10" : "hover:bg-panel"
                    }`}
                    onClick={() => setSelected(r.name)}
                  >
                    <td className="px-3 py-1.5">{r.name}</td>
                    <td className="px-3 py-1.5 text-xs text-muted">{r.caps.length}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <div className="w-1/2 min-w-0 overflow-y-auto px-4 py-3">
          <div className="space-y-3">
            <div>
              <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-muted">
                Role name
              </label>
              <input
                aria-label="role name"
                className="w-full rounded bg-panel px-2 py-1 text-sm disabled:opacity-60"
                placeholder="e.g. operator"
                value={draftName}
                disabled={selRole !== null}
                onChange={(e) => setDraftName(e.target.value)}
              />
            </div>

            <div>
              <h3 className="mb-1 text-xs font-medium uppercase tracking-wide text-muted">
                Capabilities ({draftCaps.size})
              </h3>
              {candidates.length === 0 ? (
                <p className="text-xs text-muted">
                  You hold no capabilities to bundle (no-widening).
                </p>
              ) : (
                <ul className="space-y-1">
                  {candidates.map((cap) => (
                    <li key={cap} className="flex items-center gap-2 text-xs">
                      <input
                        type="checkbox"
                        id={`cap-${cap}`}
                        aria-label={`include ${cap}`}
                        checked={draftCaps.has(cap)}
                        onChange={() => toggle(cap)}
                      />
                      <label htmlFor={`cap-${cap}`} className="font-mono">
                        {cap}
                      </label>
                    </li>
                  ))}
                </ul>
              )}
            </div>

            <button
              aria-label="save role"
              className="rounded bg-accent/15 px-3 py-1 text-xs text-accent disabled:opacity-40"
              disabled={!draftName.trim()}
              onClick={() => void save()}
            >
              {selRole ? "Save changes" : "Create role"}
            </button>
          </div>
        </div>
      </div>
    </AdminPanel>
  );
}
