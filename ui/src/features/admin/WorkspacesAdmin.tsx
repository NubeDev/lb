// Workspaces administration (admin-console scope): list with status; rename; archive (reversible
// soft-delete); guarded hard-delete (purge). Archive routes through ConfirmDestructive (reversible,
// single confirm); purge routes through it with TYPE-THE-NAME escalation — and the backend ALSO
// requires the `workspace.purge` cap + a confirm token == the ws id (defense in depth, admin-crud
// session). Markup + wiring only; data lives in useWorkspacesAdmin.

import { useState } from "react";
import { Building2 } from "lucide-react";

import { ConfirmDestructive } from "@/features/confirm";
import { useWorkspacesAdmin } from "./useWorkspacesAdmin";

type Pending = { kind: "archive" | "purge"; ws: string } | null;

interface Props {
  ws: string;
}

export function WorkspacesAdmin({ ws }: Props) {
  const { workspaces, error, archive, purge } = useWorkspacesAdmin();
  const [pending, setPending] = useState<Pending>(null);

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Building2 size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Workspaces</h1>
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="bg-panel px-4 py-2 text-xs text-accent">
          {error}
        </div>
      )}

      <ul className="flex-1 overflow-y-auto px-4 py-2">
        {workspaces.length === 0 ? (
          <li className="text-sm text-muted">No workspaces in the directory.</li>
        ) : (
          workspaces.map((w) => (
            <li key={w.ws} className="flex items-center gap-2 py-1 text-sm" role="listitem">
              <span>{w.ws}</span>
              <span className="text-xs text-muted">{w.name}</span>
              <button
                aria-label={`archive ${w.ws}`}
                className="ml-auto rounded bg-panel px-2 py-0.5 text-xs"
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
            </li>
          ))
        )}
      </ul>

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
    </section>
  );
}
