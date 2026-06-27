// The members/teams view — lists a team's members and adds one (collaboration scope, slice 3).
// Minimal: a team selector (free-text, since teams are implicit edges), the roster, and an add box.
// Markup + wiring only; data lives in useMembers.

import { useState } from "react";
import { UserPlus, Users } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
    <section className="flex h-full min-w-0 flex-col bg-bg text-fg">
      <AppPageHeader
        icon={Users}
        title="Members"
        description="Team membership inside the current workspace."
        workspace={ws}
        actions={
          <label className="flex items-center gap-2 text-xs text-muted">
            <span>Team</span>
            <Input
              aria-label="team"
              className="h-8 w-28 px-2 text-xs"
              value={team}
              onChange={(e) => setTeam(e.target.value)}
            />
          </label>
        }
      />

      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-sm text-destructive"
        >
          {error}
        </div>
      )}

      <ul className="min-h-0 flex-1 overflow-y-auto px-4 py-2">
        {members.length === 0 ? (
          <li className="text-sm text-muted">No members in {team} yet.</li>
        ) : (
          members.map((m) => (
            <li key={m} className="border-b border-border py-2 text-sm last:border-b-0" role="listitem">
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
        <Input
          aria-label="add member"
          className="min-w-0 flex-1"
          placeholder="user:… to add to the team"
          value={newUser}
          onChange={(e) => setNewUser(e.target.value)}
        />
        <Button aria-label="add">
          <UserPlus size={14} /> Add
        </Button>
      </form>
    </section>
  );
}
