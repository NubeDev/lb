// People administration (global-identity scope, decision #9) — the workspace's **membership roster**,
// re-pointed from the legacy workspace-scoped `user_list` to `membership.list` (the global-identity
// proving surface). Left: a selectable table of members (the workspaces's effective roster) with a
// "New user" toolbar. Right: the selected member's detail — teams (from the directory), the shared
// AccessEditor (roles + caps), and remove. The roster is the effective set (membership rows ∪ legacy
// user rows), so an upgraded workspace loses nobody. Every destructive verb routes through
// ConfirmDestructive; the gateway re-checks.
//
// Built on shadcn primitives (ui-standards-scope): every button, input, table cell comes from
// `components/ui/*`. NO `AppPage`/header here — this view is mounted inside `AdminView`'s tab strip,
// which already owns the page header; a second header would double up. Tokens only — destructive
// actions use the `Button`/`Badge` `destructive` variant, never `red-…` literals. The two-region
// body stacks on phone-width (`flex-col md:flex-row`, no fixed `w-1/2`).

import { useState } from "react";
import { Plus, Users } from "lucide-react";

import { AppEmptyState } from "@/components/app/empty-state";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
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
import { EffectiveCaps } from "./access/EffectiveCaps";
import { useDirectory } from "./useDirectory";
import { useMembers } from "./useMembers";
import { useRoles } from "./useRoles";

type Pending = { kind: "remove"; sub: string } | null;

interface Props {
  /** The workspace is shown in the parent `AdminView`'s header; kept on the prop for API compat. */
  ws?: string;
  /** The admin's session caps — gates the effective-caps detail + the revoke lever. */
  caps?: string[] | undefined;
}

/** Strip the `user:` prefix for display (the roster key is the full handle `user:ada`). */
function bare(sub: string): string {
  return sub.startsWith("user:") ? sub.slice("user:".length) : sub;
}

export function PeopleAdmin({ caps }: Props) {
  const { members, error, add, remove } = useMembers();
  const { teamsByUser } = useDirectory();
  const { roles } = useRoles();
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newUser, setNewUser] = useState("");
  const [pending, setPending] = useState<Pending>(null);
  const [filter, setFilter] = useState("");

  const sel = members.find((m) => m.sub === selected) ?? null;
  const roleNames = roles.map((r) => r.name);
  const visibleMembers = members.filter((m) => bare(m.sub).toLowerCase().includes(filter.toLowerCase()));

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
        {/* Left: the roster of members. Stacks above the detail on a phone. */}
        <div className="flex min-w-0 flex-1 flex-col border-b border-border md:max-w-[32rem] md:border-b-0 md:border-r">
          <AdminToolbar
            search={filter}
            onSearch={setFilter}
            searchPlaceholder="Filter members…"
            action={
              <Button
                variant={!creating ? "default" : "outline"}
                size="sm"
                aria-label="new user"
                onClick={() => setCreating((c) => !c)}
              >
                <Plus size={13} /> {creating ? "Cancel" : "New user"}
              </Button>
            }
          />
          {creating && (
            <form
              className="flex gap-2 border-b border-border bg-panel px-3 py-2"
              onSubmit={(e) => {
                e.preventDefault();
                const id = newUser.trim();
                if (id) {
                  void add(id.startsWith("user:") ? id : `user:${id}`);
                  setNewUser("");
                  setCreating(false);
                }
              }}
            >
              <Input
                autoFocus
                aria-label="new user id"
                className="h-8"
                placeholder="user id"
                value={newUser}
                onChange={(e) => setNewUser(e.target.value)}
              />
              <Button aria-label="create user" size="sm" type="submit">
                Create
              </Button>
            </form>
          )}
          <div className="min-h-0 flex-1 overflow-y-auto">
          {members.length === 0 && !creating ? (
            <AppEmptyState
              icon={Users}
              title="No members yet."
              description="Add a user to provision a global identity and pull it into this workspace."
            />
          ) : (
            <Table>
              <TableHeader sticky>
                <TableRow>
                  <TableHead>Member</TableHead>
                  <TableHead>Teams</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {visibleMembers.map((m) => (
                  <TableRow
                    key={m.sub}
                    aria-label={`select ${bare(m.sub)}`}
                    aria-selected={selected === m.sub}
                    data-state={selected === m.sub ? "selected" : undefined}
                    className="cursor-pointer"
                    onClick={() => setSelected(m.sub)}
                  >
                    <TableCell className="font-medium text-fg">{bare(m.sub)}</TableCell>
                    <TableCell className="text-muted">
                      {(teamsByUser[bare(m.sub)] ?? []).join(", ") || "—"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          </div>
        </div>

        {/* Right: the selected member's detail. */}
        <div className="min-w-0 flex-1 overflow-y-auto px-4 py-4">
          {!sel ? (
            <AppEmptyState
              icon={Users}
              title="No member selected."
              description="Select a member to see their teams, roles, and access."
            />
          ) : (
            <div className="space-y-5">
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-semibold text-fg">{bare(sel.sub)}</h2>
                <Button
                  variant="destructive"
                  size="sm"
                  className="ml-auto"
                  aria-label={`remove ${bare(sel.sub)}`}
                  onClick={() => setPending({ kind: "remove", sub: sel.sub })}
                >
                  Remove
                </Button>
              </div>

              <div>
                <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-muted">
                  Teams
                </h3>
                {(teamsByUser[bare(sel.sub)] ?? []).length === 0 ? (
                  <p className="text-xs text-muted">In no teams. Add them from the Teams tab.</p>
                ) : (
                  <ul className="flex flex-wrap gap-1.5">
                    {(teamsByUser[bare(sel.sub)] ?? []).map((t) => (
                      <li key={t}>
                        <Badge variant="secondary">{t}</Badge>
                      </li>
                    ))}
                  </ul>
                )}
              </div>

              <AccessEditor subject={sel.sub} availableRoles={roleNames} caps={caps} />
              <EffectiveCaps subject={sel.sub} />
            </div>
          )}
        </div>
      </div>

      {pending?.kind === "remove" && (
        <ConfirmDestructive
          title={`Remove ${bare(pending.sub)}`}
          consequence={`Drops ${bare(pending.sub)}'s membership AND revokes their grants + live token in this workspace. Their global identity is untouched; they may still belong to other workspaces.`}
          reversible={false}
          escalation="type-name"
          confirmName={bare(pending.sub)}
          confirmLabel="Remove"
          onConfirm={() => {
            void remove(pending.sub);
            if (selected === pending.sub) setSelected(null);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}
