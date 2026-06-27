// Members administration (admin-console scope): per team, list / add / remove members. Completes the
// collaboration MembersView with the missing destructive remove, which routes through the shared
// ConfirmDestructive and surfaces the freshness asymmetry: team-shared docs become unreadable
// IMMEDIATELY (gate 3 is live), but the user's inherited caps drop only on the NEXT sign-in (gate 2 is
// the cached token half — admin-crud / authz-grants). Markup + wiring only; data lives in
// useMembersAdmin. The gateway re-checks every verb; the UI gate is convenience.

import { useState } from "react";
import { UserMinus, UserPlus, Users } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { useMembersAdmin } from "./useMembersAdmin";

interface Props {
  ws: string;
}

export function MembersAdmin({ ws }: Props) {
  const [team, setTeam] = useState("eng");
  const { members, error, add, remove } = useMembersAdmin(team);
  const [newUser, setNewUser] = useState("");
  const [pending, setPending] = useState<string | null>(null);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Users size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Members</h1>
        <input
          aria-label="team"
          className="ml-2 rounded bg-panel px-2 py-1 text-xs"
          value={team}
          onChange={(e) => setTeam(e.target.value)}
        />
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error}
        </div>
      )}

      <ul className="flex-1 overflow-y-auto px-4 py-2">
        {members.length === 0 ? (
          <li className="text-sm text-muted">No members in {team} yet.</li>
        ) : (
          members.map((m) => (
            <li key={m} className="flex items-center gap-2 py-1 text-sm" role="listitem">
              <span>{m}</span>
              <button
                aria-label={`remove ${m}`}
                className="ml-auto flex items-center gap-1 rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                onClick={() => setPending(m)}
              >
                <UserMinus size={12} /> Remove
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
            void add(user);
            setNewUser("");
          }
        }}
      >
        <input
          aria-label="add member"
          className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-sm"
          placeholder="user:… to add to the team"
          value={newUser}
          onChange={(e) => setNewUser(e.target.value)}
        />
        <button aria-label="add" className="flex items-center gap-1 rounded bg-accent/15 px-3 text-accent">
          <UserPlus size={14} /> Add
        </button>
      </form>

      {pending && (
        <ConfirmDestructive
          title={`Remove ${pending} from ${team}`}
          consequence={`Team-shared docs become unreadable immediately; ${pending}'s inherited caps drop on next sign-in.`}
          reversible
          escalation="none"
          confirmLabel="Remove"
          onConfirm={() => {
            void remove(pending);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </section>
  );
}
