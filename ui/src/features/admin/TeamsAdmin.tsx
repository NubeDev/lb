// Teams administration (admin-console redesign) — folds in the old separate MembersAdmin so you no
// longer type a team id to see who's in it. Left: a selectable table of teams (member count). Right:
// the selected team's members (add/remove inline) + the shared AccessEditor for the team's roles &
// caps (team-inherited access). Create is a header action; delete shows the cascade consequence and
// routes through ConfirmDestructive. Data lives in useDirectory (one refresh source); the gateway
// re-checks every verb.

import { useState } from "react";
import { UserMinus, UserPlus, UsersRound } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { AdminPanel } from "./AdminPanel";
import { AccessEditor } from "./AccessEditor";
import { useDirectory } from "./useDirectory";
import { useRoles } from "./useRoles";

type Pending =
  | { kind: "deleteTeam"; team: string; count: number }
  | { kind: "removeMember"; team: string; user: string }
  | null;

interface Props {
  ws: string;
}

/** Strip the `user:` prefix the members api returns. */
function bare(id: string): string {
  return id.startsWith("user:") ? id.slice("user:".length) : id;
}

export function TeamsAdmin({ ws }: Props) {
  const {
    teams,
    membersByTeam,
    error,
    createTeamRecord,
    removeTeamRecord,
    addTeamMember,
    removeTeamMember,
  } = useDirectory();
  const { roles } = useRoles();
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newTeam, setNewTeam] = useState("");
  const [newMember, setNewMember] = useState("");
  const [pending, setPending] = useState<Pending>(null);

  const members = selected ? membersByTeam[selected] ?? [] : [];
  const roleNames = roles.map((r) => r.name);

  const action = (
    <button
      aria-label="new team"
      className="flex items-center gap-1 rounded bg-accent/15 px-2 py-1 text-xs text-accent"
      onClick={() => setCreating((c) => !c)}
    >
      <UsersRound size={13} /> New team
    </button>
  );

  return (
    <AdminPanel icon={UsersRound} title="Teams" ws={ws} action={action} error={error}>
      <div className="flex h-full">
        <div className="w-1/2 min-w-0 border-r border-border">
          {creating && (
            <form
              className="flex gap-1 border-b border-border px-3 py-2"
              onSubmit={(e) => {
                e.preventDefault();
                const team = newTeam.trim();
                if (team) {
                  void createTeamRecord(team);
                  setNewTeam("");
                  setCreating(false);
                }
              }}
            >
              <input
                autoFocus
                aria-label="new team id"
                className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-sm"
                placeholder="team id"
                value={newTeam}
                onChange={(e) => setNewTeam(e.target.value)}
              />
              <button className="rounded bg-accent/15 px-3 text-xs text-accent">Create</button>
            </form>
          )}
          {teams.length === 0 ? (
            <p className="px-4 py-3 text-sm text-muted">No teams yet.</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs text-muted">
                  <th className="px-3 py-1.5 font-medium">Team</th>
                  <th className="px-3 py-1.5 font-medium">Members</th>
                </tr>
              </thead>
              <tbody>
                {teams.map((t) => (
                  <tr
                    key={t.team}
                    aria-label={`select ${t.team}`}
                    aria-selected={selected === t.team}
                    className={`cursor-pointer border-b border-border/50 ${
                      selected === t.team ? "bg-accent/10" : "hover:bg-panel"
                    }`}
                    onClick={() => setSelected(t.team)}
                  >
                    <td className="px-3 py-1.5">{t.team}</td>
                    <td className="px-3 py-1.5 text-xs text-muted">
                      {(membersByTeam[t.team] ?? []).length}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <div className="w-1/2 min-w-0 overflow-y-auto px-4 py-3">
          {!selected ? (
            <p className="text-sm text-muted">Select a team to see its members and access.</p>
          ) : (
            <div className="space-y-4">
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-medium">{selected}</h2>
                <button
                  aria-label={`delete team ${selected}`}
                  className="ml-auto rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                  onClick={() =>
                    setPending({ kind: "deleteTeam", team: selected, count: members.length })
                  }
                >
                  Delete team
                </button>
              </div>

              <div>
                <h3 className="mb-1 text-xs font-medium uppercase tracking-wide text-muted">
                  Members
                </h3>
                {members.length === 0 ? (
                  <p className="text-xs text-muted">No members yet.</p>
                ) : (
                  <ul className="space-y-1">
                    {members.map((m) => (
                      <li key={m} className="flex items-center gap-2 text-sm">
                        <span>{bare(m)}</span>
                        <button
                          aria-label={`remove ${bare(m)}`}
                          className="ml-auto flex items-center gap-1 rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                          onClick={() =>
                            setPending({ kind: "removeMember", team: selected, user: bare(m) })
                          }
                        >
                          <UserMinus size={12} /> Remove
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
                <form
                  className="mt-2 flex gap-1"
                  onSubmit={(e) => {
                    e.preventDefault();
                    const user = newMember.trim();
                    if (user) {
                      void addTeamMember(selected, user);
                      setNewMember("");
                    }
                  }}
                >
                  <input
                    aria-label="add member"
                    className="min-w-0 flex-1 rounded bg-panel px-2 py-1 text-sm"
                    placeholder="user id to add"
                    value={newMember}
                    onChange={(e) => setNewMember(e.target.value)}
                  />
                  <button
                    aria-label="add member to team"
                    className="flex items-center gap-1 rounded bg-accent/15 px-3 text-xs text-accent"
                  >
                    <UserPlus size={13} /> Add
                  </button>
                </form>
              </div>

              <AccessEditor subject={`team:${selected}`} availableRoles={roleNames} />
            </div>
          )}
        </div>
      </div>

      {pending?.kind === "deleteTeam" && (
        <ConfirmDestructive
          title={`Delete team ${pending.team}`}
          consequence={`Removes ${pending.count} member${pending.count === 1 ? "" : "s"} and revokes the team's inherited caps (cascade). Team-shared docs become unreadable for former members immediately; their inherited caps drop on next sign-in.`}
          reversible={false}
          escalation="none"
          confirmLabel="Delete team"
          onConfirm={() => {
            void removeTeamRecord(pending.team);
            if (selected === pending.team) setSelected(null);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
      {pending?.kind === "removeMember" && (
        <ConfirmDestructive
          title={`Remove ${pending.user} from ${pending.team}`}
          consequence={`Team-shared docs become unreadable immediately; ${pending.user}'s inherited caps drop on next sign-in.`}
          reversible
          escalation="none"
          confirmLabel="Remove"
          onConfirm={() => {
            void removeTeamMember(pending.team, pending.user);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </AdminPanel>
  );
}
