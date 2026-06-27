// Users administration (admin-console scope): list users with active status; create (seeds a dev
// credential); disable/enable; delete (with the grant-revocation consequence shown). Every
// destructive action (disable, delete) routes through the shared ConfirmDestructive — never a bespoke
// confirm. Markup + wiring only; data lives in useUsersAdmin. Per-control cap-gating is the caller's
// (App shows the section only to an admin); the GATEWAY re-checks every verb — the UI gate is
// convenience, not the boundary (asserted in the test via a forged-call deny on the Rust side).

import { useState } from "react";
import { UserPlus, Users } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { useUsersAdmin } from "./useUsersAdmin";

type Pending =
  | { kind: "disable"; user: string }
  | { kind: "delete"; user: string }
  | null;

interface Props {
  ws: string;
}

export function UsersAdmin({ ws }: Props) {
  const { users, error, create, setActive, remove } = useUsersAdmin();
  const [newUser, setNewUser] = useState("");
  const [pending, setPending] = useState<Pending>(null);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Users size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Users</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error}
        </div>
      )}

      <ul className="flex-1 overflow-y-auto px-4 py-2">
        {users.length === 0 ? (
          <li className="text-sm text-muted">No users yet.</li>
        ) : (
          users.map((u) => (
            <li key={u.user} className="flex items-center gap-2 py-1 text-sm" role="listitem">
              <span>{u.user}</span>
              <span className={`text-xs ${u.active ? "text-accent" : "text-muted"}`}>
                {u.active ? "active" : "disabled"}
              </span>
              <button
                aria-label={`${u.active ? "disable" : "enable"} ${u.user}`}
                className="ml-auto rounded bg-panel px-2 py-0.5 text-xs"
                onClick={() =>
                  u.active ? setPending({ kind: "disable", user: u.user }) : void setActive(u.user, true)
                }
              >
                {u.active ? "Disable" : "Enable"}
              </button>
              <button
                aria-label={`delete ${u.user}`}
                className="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                onClick={() => setPending({ kind: "delete", user: u.user })}
              >
                Delete
              </button>
            </li>
          ))
        )}
      </ul>

      <form
        className="flex gap-1 border-t border-border px-4 py-2"
        onSubmit={(e) => {
          e.preventDefault();
          const user = newUser.trim();
          if (user) {
            void create(user);
            setNewUser("");
          }
        }}
      >
        <input
          aria-label="new user"
          className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-sm"
          placeholder="user:… to create"
          value={newUser}
          onChange={(e) => setNewUser(e.target.value)}
        />
        <button aria-label="create user" className="flex items-center gap-1 rounded bg-accent/15 px-3 text-accent">
          <UserPlus size={14} /> Create
        </button>
      </form>

      {pending?.kind === "disable" && (
        <ConfirmDestructive
          title={`Disable ${pending.user}`}
          consequence={`${pending.user} cannot sign in until re-enabled. Active sessions keep their cached caps until the token expires (re-enable restores access).`}
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
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </section>
  );
}
