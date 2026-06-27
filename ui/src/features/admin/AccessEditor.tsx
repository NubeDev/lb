// The access editor (admin-console redesign) — shows and edits ONE subject's access: the roles
// assigned to it (the primary, human path: pick a named role) and, behind an "advanced" toggle, the
// raw capability strings (the power-user path). Used for both a user (`user:bob`) and a team
// (`team:eng`) so the two tabs share one editor. Assigning a role is a grant of `role:<name>`; the
// gateway re-checks every verb and enforces no-widening — the UI is convenience. Revokes route
// through the shared ConfirmDestructive (reversible). Markup + wiring; data in useSubjectGrants.

import { useState } from "react";

import { ConfirmDestructive } from "@/features/confirm";
import { useSubjectGrants } from "./useSubjectGrants";

interface Props {
  /** The `kind:name` subject, or null when nothing is selected. */
  subject: string | null;
  /** Every role defined in the workspace (for the assign dropdown). */
  availableRoles: string[];
}

type Pending = { kind: "role" | "cap"; value: string } | null;

export function AccessEditor({ subject, availableRoles }: Props) {
  const { roles, caps, error, assignRole, revokeRole, assignCap, revokeCap } =
    useSubjectGrants(subject);
  const [pick, setPick] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [newCap, setNewCap] = useState("");
  const [pending, setPending] = useState<Pending>(null);

  if (!subject) return null;
  const unassigned = availableRoles.filter((r) => !roles.includes(r));

  return (
    <div className="space-y-4">
      {error && (
        <div role="alert" className="text-xs text-accent">
          {error}
        </div>
      )}

      <div>
        <h3 className="mb-1 text-xs font-medium uppercase tracking-wide text-muted">Roles</h3>
        {roles.length === 0 ? (
          <p className="text-xs text-muted">No roles assigned.</p>
        ) : (
          <ul className="flex flex-wrap gap-1.5">
            {roles.map((r) => (
              <li
                key={r}
                className="flex items-center gap-1 rounded bg-accent/15 px-2 py-0.5 text-xs text-accent"
              >
                {r}
                <button
                  aria-label={`revoke role ${r} from ${subject}`}
                  className="text-accent/70 hover:text-accent"
                  onClick={() => setPending({ kind: "role", value: r })}
                >
                  ×
                </button>
              </li>
            ))}
          </ul>
        )}
        {unassigned.length > 0 && (
          <div className="mt-2 flex gap-1">
            <select
              aria-label={`assign a role to ${subject}`}
              className="rounded bg-panel px-2 py-1 text-xs"
              value={pick}
              onChange={(e) => setPick(e.target.value)}
            >
              <option value="">Assign a role…</option>
              {unassigned.map((r) => (
                <option key={r} value={r}>
                  {r}
                </option>
              ))}
            </select>
            <button
              aria-label="assign role"
              className="rounded bg-accent/15 px-2 py-1 text-xs text-accent disabled:opacity-40"
              disabled={!pick}
              onClick={() => {
                void assignRole(pick);
                setPick("");
              }}
            >
              Assign
            </button>
          </div>
        )}
      </div>

      <div>
        <button
          className="text-xs text-muted hover:text-fg"
          onClick={() => setShowAdvanced((s) => !s)}
        >
          {showAdvanced ? "▾" : "▸"} Advanced: direct capabilities ({caps.length})
        </button>
        {showAdvanced && (
          <div className="mt-2 space-y-2">
            {caps.length === 0 ? (
              <p className="text-xs text-muted">No direct capabilities.</p>
            ) : (
              <ul className="space-y-1">
                {caps.map((c) => (
                  <li key={c} className="flex items-center gap-2 text-xs">
                    <span className="font-mono">{c}</span>
                    <button
                      aria-label={`revoke ${c}`}
                      className="ml-auto rounded bg-red-500/15 px-2 py-0.5 text-red-400"
                      onClick={() => setPending({ kind: "cap", value: c })}
                    >
                      Revoke
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <form
              className="flex gap-1"
              onSubmit={(e) => {
                e.preventDefault();
                const cap = newCap.trim();
                if (cap) {
                  void assignCap(cap);
                  setNewCap("");
                }
              }}
            >
              <input
                aria-label="capability to assign"
                className="min-w-0 flex-1 rounded bg-panel px-2 py-1 font-mono text-xs"
                placeholder="mcp:…:call"
                value={newCap}
                onChange={(e) => setNewCap(e.target.value)}
              />
              <button aria-label="assign capability" className="rounded bg-accent/15 px-3 text-xs text-accent">
                Assign
              </button>
            </form>
          </div>
        )}
      </div>

      {pending && (
        <ConfirmDestructive
          title={pending.kind === "role" ? `Revoke role ${pending.value}` : `Revoke ${pending.value}`}
          consequence={`Removes this ${pending.kind === "role" ? "role" : "capability"} from ${subject}. Membership-checked access drops immediately; the cached token half drops on the subject's next sign-in. Reversible — re-assign restores it.`}
          reversible
          escalation="none"
          confirmLabel="Revoke"
          onConfirm={() => {
            if (pending.kind === "role") void revokeRole(pending.value);
            else void revokeCap(pending.value);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}
