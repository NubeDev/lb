// Workspaces administration (admin-console scope): list with status; rename; archive (reversible
// soft-delete); guarded hard-delete (purge). Archive routes through ConfirmDestructive (reversible,
// single confirm); purge routes through it with TYPE-THE-NAME escalation — and the backend ALSO
// requires the `workspace.purge` cap + a confirm token == the ws id (defense in depth, admin-crud
// session). Markup + wiring only; data lives in useWorkspacesAdmin.
//
// Built on shadcn primitives (access-console consistency): the shared `Table` (sticky header) + the
// shared `AdminToolbar` (search), no local page header (the `AdminView` tab strip owns it), no raw
// `<table>`/`<button>`. Tokens only — the guarded purge uses the `Button` `destructive` variant,
// never a `red-…` literal.

import { useState } from "react";
import { Building2 } from "lucide-react";

import { AppEmptyState } from "@/components/app/empty-state";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfirmDestructive } from "@/features/confirm";
import { AdminToolbar } from "./AdminToolbar";
import { useWorkspacesAdmin } from "./useWorkspacesAdmin";

type Pending = { kind: "archive" | "purge"; ws: string } | null;

interface Props {
  /** The workspace is shown in the parent `AdminView`'s header; kept on the prop for API compat. */
  ws?: string;
}

export function WorkspacesAdmin(_: Props) {
  const { workspaces, error, archive, purge } = useWorkspacesAdmin();
  const [pending, setPending] = useState<Pending>(null);
  const [filter, setFilter] = useState("");

  const q = filter.toLowerCase();
  const visible = workspaces.filter(
    (w) => w.ws.toLowerCase().includes(q) || w.name.toLowerCase().includes(q),
  );

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

      <AdminToolbar search={filter} onSearch={setFilter} searchPlaceholder="Filter workspaces…" />

      <div className="min-h-0 flex-1 overflow-y-auto">
        {visible.length === 0 ? (
          <AppEmptyState
            icon={Building2}
            title={filter ? "No workspaces match." : "No workspaces in the directory."}
            description={filter ? "Clear the filter to see every workspace." : ""}
          />
        ) : (
          <Table>
            <TableHeader sticky>
              <TableRow>
                <TableHead>Workspace</TableHead>
                <TableHead>Name</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {visible.map((w) => (
                <TableRow key={w.ws} role="listitem">
                  <TableCell className="font-medium text-fg">{w.ws}</TableCell>
                  <TableCell className="text-muted">{w.name}</TableCell>
                  <TableCell className="text-right">
                    <div className="flex justify-end gap-1">
                      <Button
                        variant="outline"
                        size="sm"
                        aria-label={`archive ${w.ws}`}
                        onClick={() => setPending({ kind: "archive", ws: w.ws })}
                      >
                        Archive
                      </Button>
                      <Button
                        variant="destructive"
                        size="sm"
                        aria-label={`purge ${w.ws}`}
                        onClick={() => setPending({ kind: "purge", ws: w.ws })}
                      >
                        Purge
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>

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
    </div>
  );
}
