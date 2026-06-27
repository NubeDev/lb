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
      <header className="page-header">
        <div className="page-header-icon">
          <Users size={16} />
        </div>
        <div className="min-w-0">
          <h1 className="page-title">Members</h1>
          <p className="page-subtitle">Team membership inside the current workspace.</p>
        </div>
        <input
          aria-label="team"
          className="control-field-sm ml-2 w-28"
          value={team}
          onChange={(e) => setTeam(e.target.value)}
        />
        <span className="scope-pill ml-auto" title={`Workspace ${ws}`}>
          <span className="h-1.5 w-1.5 rounded-full bg-accent" aria-hidden />
          <span className="truncate">{ws}</span>
        </span>
      </header>

      {error && (
        <div role="alert" className="state-alert">
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
          className="control-field min-w-0 flex-1"
          placeholder="user:… to add to the team"
          value={newUser}
          onChange={(e) => setNewUser(e.target.value)}
        />
        <button aria-label="add" className="soft-button">
          <UserPlus size={14} /> Add
        </button>
      </form>
    </section>
  );
}
