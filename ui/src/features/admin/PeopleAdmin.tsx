// People administration (admin-console redesign) — the relationship-first replacement for the old
// flat Users list. Left: a selectable table of users (status · the teams they're in · role count).
// Right: the selected user's detail — status, enable/disable, delete, their teams, and the shared
// AccessEditor (roles + advanced caps). This answers "who belongs to who" without typing ids: teams
// come from useDirectory's inverted membership map. Create is a header action (an inline row), never
// a chat composer. Every destructive verb routes through ConfirmDestructive; the gateway re-checks.

import { useState } from "react";
import { UserPlus, Users } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { AdminPanel } from "./AdminPanel";
import { AccessEditor } from "./AccessEditor";
import { useDirectory } from "./useDirectory";
import { useRoles } from "./useRoles";

type Pending = { kind: "disable" | "delete"; user: string } | null;

interface Props {
  ws: string;
}

export function PeopleAdmin({ ws }: Props) {
  const { users, teamsByUser, error, create, setActive, remove } = useDirectory();
  const { roles } = useRoles();
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newUser, setNewUser] = useState("");
  const [pending, setPending] = useState<Pending>(null);

  const sel = users.find((u) => u.user === selected) ?? null;
  const roleNames = roles.map((r) => r.name);

  const action = (
    <button
      aria-label="new user"
      className="flex items-center gap-1 rounded bg-accent/15 px-2 py-1 text-xs text-accent"
      onClick={() => setCreating((c) => !c)}
    >
      <UserPlus size={13} /> New user
    </button>
  );

  return (
    <AdminPanel icon={Users} title="People" ws={ws} action={action} error={error}>
      <div className="flex h-full">
        <div className="w-1/2 min-w-0 border-r border-border">
          {creating && (
            <form
              className="flex gap-1 border-b border-border px-3 py-2"
              onSubmit={(e) => {
                e.preventDefault();
                const user = newUser.trim();
                if (user) {
                  void create(user);
                  setNewUser("");
                  setCreating(false);
                }
              }}
            >
              <input
                autoFocus
                aria-label="new user id"
                className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-sm"
                placeholder="user id"
                value={newUser}
                onChange={(e) => setNewUser(e.target.value)}
              />
              <button className="rounded bg-accent/15 px-3 text-xs text-accent">Create</button>
            </form>
          )}
          {users.length === 0 ? (
            <p className="px-4 py-3 text-sm text-muted">No users yet.</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs text-muted">
                  <th className="px-3 py-1.5 font-medium">User</th>
                  <th className="px-3 py-1.5 font-medium">Status</th>
                  <th className="px-3 py-1.5 font-medium">Teams</th>
                </tr>
              </thead>
              <tbody>
                {users.map((u) => (
                  <tr
                    key={u.user}
                    aria-label={`select ${u.user}`}
                    aria-selected={selected === u.user}
                    className={`cursor-pointer border-b border-border/50 ${
                      selected === u.user ? "bg-accent/10" : "hover:bg-panel"
                    }`}
                    onClick={() => setSelected(u.user)}
                  >
                    <td className="px-3 py-1.5">{u.user}</td>
                    <td className="px-3 py-1.5">
                      <span className={`text-xs ${u.active ? "text-accent" : "text-muted"}`}>
                        {u.active ? "active" : "disabled"}
                      </span>
                    </td>
                    <td className="px-3 py-1.5 text-xs text-muted">
                      {(teamsByUser[u.user] ?? []).join(", ") || "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <div className="w-1/2 min-w-0 overflow-y-auto px-4 py-3">
          {!sel ? (
            <p className="text-sm text-muted">Select a user to see their teams, roles, and access.</p>
          ) : (
            <div className="space-y-4">
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-medium">{sel.user}</h2>
                <span className={`text-xs ${sel.active ? "text-accent" : "text-muted"}`}>
                  {sel.active ? "active" : "disabled"}
                </span>
                <div className="ml-auto flex gap-1">
                  <button
                    aria-label={`${sel.active ? "disable" : "enable"} ${sel.user}`}
                    className="rounded bg-panel px-2 py-0.5 text-xs"
                    onClick={() =>
                      sel.active
                        ? setPending({ kind: "disable", user: sel.user })
                        : void setActive(sel.user, true)
                    }
                  >
                    {sel.active ? "Disable" : "Enable"}
                  </button>
                  <button
                    aria-label={`delete ${sel.user}`}
                    className="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                    onClick={() => setPending({ kind: "delete", user: sel.user })}
                  >
                    Delete
                  </button>
                </div>
              </div>

              <div>
                <h3 className="mb-1 text-xs font-medium uppercase tracking-wide text-muted">Teams</h3>
                {(teamsByUser[sel.user] ?? []).length === 0 ? (
                  <p className="text-xs text-muted">In no teams. Add them from the Teams tab.</p>
                ) : (
                  <ul className="flex flex-wrap gap-1.5">
                    {(teamsByUser[sel.user] ?? []).map((t) => (
                      <li key={t} className="rounded bg-panel px-2 py-0.5 text-xs text-muted">
                        {t}
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              <AccessEditor subject={`user:${sel.user}`} availableRoles={roleNames} />
            </div>
          )}
        </div>
      </div>

      {pending?.kind === "disable" && (
        <ConfirmDestructive
          title={`Disable ${pending.user}`}
          consequence={`${pending.user} cannot sign in until re-enabled. Active sessions keep cached caps until the token expires.`}
          reversible
          escalation="none"
          confirmLabel="Disable"
          onConfirm={() => {
            void setActive(pending.user, false);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
      {pending?.kind === "delete" && (
        <ConfirmDestructive
          title={`Delete ${pending.user}`}
          consequence={`Tombstones ${pending.user} and revokes ALL their grants. They can no longer sign in; team-inherited access drops on next sign-in.`}
          reversible={false}
          escalation="type-name"
          confirmName={pending.user}
          confirmLabel="Delete"
          onConfirm={() => {
            void remove(pending.user);
            if (selected === pending.user) setSelected(null);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </AdminPanel>
  );
}
