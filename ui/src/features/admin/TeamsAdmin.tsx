// Teams administration (admin-console scope): list team records; create; rename; delete (showing the
// member count + the cascade consequence). Delete routes through the shared ConfirmDestructive. The
// member count is read live (listMembers) when the confirm opens so the consequence text is accurate
// — stale cascade copy misleads admins (a scope risk). Markup + wiring only; data lives in
// useTeamsAdmin. The gateway re-checks every verb; the UI gate is convenience.

import { useState } from "react";
import { UsersRound } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { listMembers } from "@/lib/members/members.api";
import { useTeamsAdmin } from "./useTeamsAdmin";

interface Props {
  ws: string;
}

export function TeamsAdmin({ ws }: Props) {
  const { teams, error, create, rename, remove } = useTeamsAdmin();
  const [newTeam, setNewTeam] = useState("");
  const [pending, setPending] = useState<{ team: string; count: number } | null>(null);

  async function openDelete(team: string) {
    let count = 0;
    try {
      count = (await listMembers(team)).length;
    } catch {
      count = 0;
    }
    setPending({ team, count });
  }

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <UsersRound size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Teams</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error}
        </div>
      )}

      <ul className="flex-1 overflow-y-auto px-4 py-2">
        {teams.length === 0 ? (
          <li className="text-sm text-muted">No teams yet.</li>
        ) : (
          teams.map((t) => (
            <li key={t.team} className="flex items-center gap-2 py-1 text-sm" role="listitem">
              <span>{t.team}</span>
              <span className="text-xs text-muted">{t.name}</span>
              <button
                aria-label={`rename ${t.team}`}
                className="ml-auto rounded bg-panel px-2 py-0.5 text-xs"
                onClick={() => void rename(t.team, `${t.name}*`)}
              >
                Rename
              </button>
              <button
                aria-label={`delete ${t.team}`}
                className="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                onClick={() => void openDelete(t.team)}
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
          const team = newTeam.trim();
          if (team) {
            void create(team, team);
            setNewTeam("");
          }
        }}
      >
        <input
          aria-label="new team"
          className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-sm"
          placeholder="team id to create"
          value={newTeam}
          onChange={(e) => setNewTeam(e.target.value)}
        />
        <button aria-label="create team" className="rounded bg-accent/15 px-3 text-accent">
          Create
        </button>
      </form>

      {pending && (
        <ConfirmDestructive
          title={`Delete team ${pending.team}`}
          consequence={`Removes ${pending.count} member${pending.count === 1 ? "" : "s"} and revokes the team's inherited caps (cascade). Team-shared docs become unreadable for former members immediately; their inherited caps drop on next sign-in.`}
          reversible={false}
          escalation="none"
          confirmLabel="Delete team"
          onConfirm={() => {
            void remove(pending.team);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </section>
  );
}
