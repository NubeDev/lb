// People administration (global-identity scope, decision #9) — the workspace's **membership roster**,
// re-pointed from the legacy workspace-scoped `user_list` to `membership.list` (the global-identity
// proving surface). Left: a selectable table of members (the workspaces's effective roster). Right:
// the selected member's detail — teams (from the directory), the shared AccessEditor (roles + caps),
// and remove. "New user" provisions a global identity + adds it to this workspace. The roster is the
// effective set (membership rows ∪ legacy user rows), so an upgraded workspace loses nobody. Every
// destructive verb routes through ConfirmDestructive; the gateway re-checks.

import { useState } from "react";
import { UserPlus, Users } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { AdminPanel } from "./AdminPanel";
import { AccessEditor } from "./AccessEditor";
import { EffectiveCaps } from "./access/EffectiveCaps";
import { useDirectory } from "./useDirectory";
import { useMembers } from "./useMembers";
import { useRoles } from "./useRoles";

type Pending = { kind: "remove"; sub: string } | null;

interface Props {
  ws: string;
  /** The admin's session caps — gates the effective-caps detail + the revoke lever. */
  caps?: string[] | undefined;
}

/** Strip the `user:` prefix for display (the roster key is the full handle `user:ada`). */
function bare(sub: string): string {
  return sub.startsWith("user:") ? sub.slice("user:".length) : sub;
}

export function PeopleAdmin({ ws, caps }: Props) {
  const { members, error, add, remove } = useMembers();
  const { teamsByUser } = useDirectory();
  const { roles } = useRoles();
  const [selected, setSelected] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newUser, setNewUser] = useState("");
  const [pending, setPending] = useState<Pending>(null);

  const sel = members.find((m) => m.sub === selected) ?? null;
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
                const id = newUser.trim();
                if (id) {
                  void add(id.startsWith("user:") ? id : `user:${id}`);
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
          {members.length === 0 ? (
            <p className="px-4 py-3 text-sm text-muted">No members yet.</p>
          ) : (
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs text-muted">
                  <th className="px-3 py-1.5 font-medium">Member</th>
                  <th className="px-3 py-1.5 font-medium">Teams</th>
                </tr>
              </thead>
              <tbody>
                {members.map((m) => (
                  <tr
                    key={m.sub}
                    aria-label={`select ${bare(m.sub)}`}
                    aria-selected={selected === m.sub}
                    className={`cursor-pointer border-b border-border/50 ${
                      selected === m.sub ? "bg-accent/10" : "hover:bg-panel"
                    }`}
                    onClick={() => setSelected(m.sub)}
                  >
                    <td className="px-3 py-1.5">{bare(m.sub)}</td>
                    <td className="px-3 py-1.5 text-xs text-muted">
                      {(teamsByUser[bare(m.sub)] ?? []).join(", ") || "—"}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>

        <div className="w-1/2 min-w-0 overflow-y-auto px-4 py-3">
          {!sel ? (
            <p className="text-sm text-muted">Select a member to see their teams, roles, and access.</p>
          ) : (
            <div className="space-y-4">
              <div className="flex items-center gap-2">
                <h2 className="text-sm font-medium">{bare(sel.sub)}</h2>
                <div className="ml-auto flex gap-1">
                  <button
                    aria-label={`remove ${bare(sel.sub)}`}
                    className="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                    onClick={() => setPending({ kind: "remove", sub: sel.sub })}
                  >
                    Remove
                  </button>
                </div>
              </div>

              <div>
                <h3 className="mb-1 text-xs font-medium uppercase tracking-wide text-muted">Teams</h3>
                {(teamsByUser[bare(sel.sub)] ?? []).length === 0 ? (
                  <p className="text-xs text-muted">In no teams. Add them from the Teams tab.</p>
                ) : (
                  <ul className="flex flex-wrap gap-1.5">
                    {(teamsByUser[bare(sel.sub)] ?? []).map((t) => (
                      <li key={t} className="rounded bg-panel px-2 py-0.5 text-xs text-muted">
                        {t}
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
    </AdminPanel>
  );
}
