// The members/teams view — lists a team's members and adds one (collaboration scope, slice 3).
// Minimal: a team selector (free-text, since teams are implicit edges), the roster, and an add box.
// Markup + wiring only; data lives in useMembers.

import { useState } from "react";
import { UserPlus, Users } from "lucide-react";

import { useMembers } from "./useMembers";

interface Props {
  /** The current workspace (for the header). */
  ws: string;
}

export function MembersView({ ws }: Props) {
  const [team, setTeam] = useState("eng");
  const { members, error, add } = useMembers(team);
  const [newUser, setNewUser] = useState("");

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
            <li key={m} className="py-1 text-sm" role="listitem">
              {m}
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
    </section>
  );
}
