// Workspaces administration (admin-console scope): list with status; rename; archive (reversible
// soft-delete); guarded hard-delete (purge). Archive routes through ConfirmDestructive (reversible,
// single confirm); purge routes through it with TYPE-THE-NAME escalation — and the backend ALSO
// requires the `workspace.purge` cap + a confirm token == the ws id (defense in depth, admin-crud
// session). Markup + wiring only; data lives in useWorkspacesAdmin.

import { useState } from "react";
import { Building2 } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { AdminPanel } from "./AdminPanel";
import { useWorkspacesAdmin } from "./useWorkspacesAdmin";

type Pending = { kind: "archive" | "purge"; ws: string } | null;

interface Props {
  ws: string;
}

export function WorkspacesAdmin({ ws }: Props) {
  const { workspaces, error, archive, purge } = useWorkspacesAdmin();
  const [pending, setPending] = useState<Pending>(null);

  return (
    <AdminPanel icon={Building2} title="Workspaces" ws={ws} error={error}>
      {workspaces.length === 0 ? (
        <p className="px-4 py-3 text-sm text-muted">No workspaces in the directory.</p>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left text-xs text-muted">
              <th className="px-3 py-1.5 font-medium">Workspace</th>
              <th className="px-3 py-1.5 font-medium">Name</th>
              <th className="px-3 py-1.5" />
            </tr>
          </thead>
          <tbody>
            {workspaces.map((w) => (
              <tr key={w.ws} className="border-b border-border/50" role="listitem">
                <td className="px-3 py-1.5">{w.ws}</td>
                <td className="px-3 py-1.5 text-xs text-muted">{w.name}</td>
                <td className="px-3 py-1.5">
                  <div className="flex justify-end gap-1">
                    <button
                      aria-label={`archive ${w.ws}`}
                      className="rounded bg-panel px-2 py-0.5 text-xs"
                      onClick={() => setPending({ kind: "archive", ws: w.ws })}
                    >
                      Archive
                    </button>
                    <button
                      aria-label={`purge ${w.ws}`}
                      className="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                      onClick={() => setPending({ kind: "purge", ws: w.ws })}
                    >
                      Purge
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {pending?.kind === "archive" && (
        <ConfirmDestructive
          title={`Archive ${pending.ws}`}
          consequence={`Soft-deletes ${pending.ws}: hidden from the list, no new sessions. Reversible — rename restores it. Data is retained.`}
          reversible
          escalation="none"
          confirmLabel="Archive"
          onConfirm={() => {
            void archive(pending.ws);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
      {pending?.kind === "purge" && (
        <ConfirmDestructive
          title={`Purge ${pending.ws}`}
          consequence={`Hard-deletes ${pending.ws}: ALL its data is destroyed and tombstoned — unrecoverable, the id cannot be reused. The server also requires the workspace.purge capability and a confirm token equal to the id.`}
          reversible={false}
          escalation="type-name"
          confirmName={pending.ws}
          confirmLabel="Purge"
          onConfirm={() => {
            void purge(pending.ws, pending.ws);
            setPending(null);
          }}
          onCancel={() => setPending(null)}
        />
      )}
    </AdminPanel>
  );
}
