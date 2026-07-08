// Teams administration (admin-console redesign) — folds in the old separate MembersAdmin so you no
// longer type a team id to see who's in it. Left: a selectable table of teams (member count) with a
// "New team" toolbar. Right: the selected team's members (add/remove inline) + the shared
// AccessEditor for the team's roles & caps. Delete shows the cascade consequence and routes through
// ConfirmDestructive. Data lives in useDirectory (one refresh source); the gateway re-checks every verb.
//
// Built on shadcn primitives (ui-standards-scope): every button, input, select, table cell comes
// from `components/ui/*`. NO `AppPage`/header here — this view is mounted inside `AdminView`'s tab
// strip, which already owns the page header; a second header would double up. Tokens only — no
// `red-…`/`zinc-…` literals (the destructive `Button`/`Badge` variants replace them). The two-region
// body stacks on phone-width (`flex-col md:flex-row`, no fixed `w-1/2`).

import { useState } from "react";
import { Plus, UserMinus, UserPlus, UsersRound } from "lucide-react";

import { AppEmptyState } from "@/components/app/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { AdminToolbar } from "./AdminToolbar";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfirmDestructive } from "@/features/confirm";
import { AccessEditor } from "./AccessEditor";
import { useDirectory } from "./useDirectory";
import { useRoles } from "./useRoles";

type Pending =
  | { kind: "deleteTeam"; team: string; count: number }
  | { kind: "removeMember"; team: string; user: string }
  | null;

interface Props {
  /** The workspace is shown in the parent `AdminView`'s header; kept on the prop for API compat. */
  ws?: string;
}

/** Strip the `user:` prefix the members api returns. */
function bare(id: string): string {
  return id.startsWith("user:") ? id.slice("user:".length) : id;
}

export function TeamsAdmin(_: Props) {
  const {
    users,
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
  const [filter, setFilter] = useState("");

  const visibleTeams = teams.filter((t) => t.team.toLowerCase().includes(filter.toLowerCase()));
  const members = selected ? membersByTeam[selected] ?? [] : [];
  const memberSet = new Set(members.map(bare));
  const candidates = users.filter((u) => u.active && !memberSet.has(u.user));
  const roleNames = roles.map((r) => r.name);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {error && (
        <div
          role="alert"
          className="border-b border-destructive/20 bg-destructive/10 px-4 py-2 text-xs text-destructive"
        >
          {error}
        </div>
      )}

      <div className="flex min-h-0 flex-1 flex-col md:flex-row">
        {/* Left: the roster of teams. Stacks above the detail on a phone. */}
        <div className="flex min-w-0 flex-1 flex-col border-b border-border md:max-w-[28rem] md:border-b-0 md:border-r">
          <AdminToolbar
            search={filter}
            onSearch={setFilter}
            searchPlaceholder="Filter teams…"
            action={
              <Button
                variant={!creating ? "default" : "outline"}
                size="sm"
                aria-label="new team"
                onClick={() => setCreating((c) => !c)}
              >
                {creating ? "Cancel" : (
                  <>
                    <Plus size={13} /> New team
                  </>
                )}
              </Button>
            }
          />
          {creating && (
            <form
              className="flex gap-2 border-b border-border bg-panel px-3 py-2"
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
              <Input
                autoFocus
                aria-label="new team id"
                className="h-8"
                placeholder="team id"
                value={newTeam}
                onChange={(e) => setNewTeam(e.target.value)}
              />
              <Button aria-label="create team" size="sm" type="submit">
                Create
              </Button>
            </form>
          )}
          <div className="min-h-0 flex-1 overflow-y-auto">
          {teams.length === 0 && !creating ? (
            <AppEmptyState
              icon={UsersRound}
              title="No teams yet."
              description="Create one to share access with a group of users at once."
            />
          ) : (
            <Table>
              <TableHeader sticky>
                <TableRow>
                  <TableHead>Team</TableHead>
                  <TableHead>Members</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {visibleTeams.map((t) => (
                  <TableRow
                    key={t.team}
                    aria-label={`select ${t.team}`}
                    aria-selected={selected === t.team}
                    data-state={selected === t.team ? "selected" : undefined}
                    className="cursor-pointer"
                    onClick={() => setSelected(t.team)}
                  >
                    <TableCell className="font-medium text-fg">{t.team}</TableCell>
                    <TableCell className="text-muted">
                      {(membersByTeam[t.team] ?? []).length}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          </div>
        </div>

        {/* Right: the selected team's members + inherited access. */}
        <div className="min-w-0 flex-1 overflow-y-auto px-4 py-4">
          {!selected ? (
            <AppEmptyState
              icon={UsersRound}
              title="No team selected."
              description="Select a team to see its members and inherited access."
            />
          ) : (
            <div className="space-y-5">
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-semibold text-fg">{selected}</h2>
                <Button
                  variant="destructive"
                  size="sm"
                  className="ml-auto"
                  aria-label={`delete team ${selected}`}
                  onClick={() =>
                    setPending({ kind: "deleteTeam", team: selected, count: members.length })
                  }
                >
                  Delete team
                </Button>
              </div>

              <div>
                <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-muted">
                  Members
                </h3>
                {members.length === 0 ? (
                  <p className="text-xs text-muted">No members yet.</p>
                ) : (
                  <ul className="space-y-1.5">
                    {members.map((m) => (
                      <li
                        key={m}
                        className="flex items-center gap-2 rounded-md border border-border bg-bg px-2.5 py-1.5 text-sm"
                      >
                        <Badge variant="secondary">{bare(m)}</Badge>
                        <Button
                          variant="destructive"
                          size="sm"
                          className="ml-auto"
                          aria-label={`remove ${bare(m)}`}
                          onClick={() =>
                            setPending({ kind: "removeMember", team: selected, user: bare(m) })
                          }
                        >
                          <UserMinus size={12} /> Remove
                        </Button>
                      </li>
                    ))}
                  </ul>
                )}
                <form
                  className="mt-3 flex gap-2"
                  onSubmit={(e) => {
                    e.preventDefault();
                    const user = newMember.trim();
                    if (user) {
                      void addTeamMember(selected, user);
                      setNewMember("");
                    }
                  }}
                >
                  <Select
                    aria-label="add member"
                    className="h-8 flex-1"
                    value={newMember}
                    onChange={(e) => setNewMember(e.target.value)}
                  >
                    <option value="">
                      {candidates.length === 0
                        ? "No users available to add"
                        : "Select a user to add…"}
                    </option>
                    {candidates.map((u) => (
                      <option key={u.user} value={u.user}>
                        {u.user}
                      </option>
                    ))}
                  </Select>
                  <Button
                    aria-label="add member to team"
                    size="sm"
                    type="submit"
                    disabled={!newMember}
                  >
                    <UserPlus size={13} /> Add
                  </Button>
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
    </div>
  );
}
