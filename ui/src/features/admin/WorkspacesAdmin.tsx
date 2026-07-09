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
import { Building2, Plus } from "lucide-react";

import { AppEmptyState } from "@/components/app/empty-state";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ConfirmDestructive } from "@/features/confirm";
import { CAP, hasCap } from "@/lib/session";
import { AdminToolbar } from "./AdminToolbar";
import { useWorkspacesAdmin } from "./useWorkspacesAdmin";

type Pending = { kind: "archive" | "purge"; ws: string } | null;

interface Props {
  /** The workspace is shown in the parent `AdminView`'s header; kept on the prop for API compat. */
  ws?: string;
  /** Session caps — gate the "New workspace" control for DISPLAY (the gateway re-checks the verb). */
  caps?: string[];
}

export function WorkspacesAdmin({ caps }: Props) {
  const { workspaces, error, create, archive, purge } = useWorkspacesAdmin();
  const [pending, setPending] = useState<Pending>(null);
  const [filter, setFilter] = useState("");
  const canCreate = hasCap(caps, CAP.workspaceCreate);

  // Create-form state (mirrors the API Keys "New key" flow): a toggle + id/name inputs.
  const [creating, setCreating] = useState(false);
  const [newWs, setNewWs] = useState("");
  const [newName, setNewName] = useState("");

  const q = filter.toLowerCase();
  const visible = workspaces.filter(
    (w) => w.ws.toLowerCase().includes(q) || w.name.toLowerCase().includes(q),
  );

  async function save() {
    // The id is the SurrealDB namespace (the hard wall) — slugify to keep it a safe key; the display
    // name is free text. Default the name to the id when left blank.
    const ws = newWs.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
    if (!ws) return;
    await create(ws, newName.trim() || ws);
    setNewWs("");
    setNewName("");
    setCreating(false);
  }

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

      <AdminToolbar
        search={filter}
        onSearch={setFilter}
        searchPlaceholder="Filter workspaces…"
        action={
          canCreate && (
            <Button
              variant={creating ? "outline" : "default"}
              size="sm"
              aria-label="new workspace"
              onClick={() => setCreating((c) => !c)}
            >
              {creating ? "Cancel" : (
                <>
                  <Plus size={13} /> New workspace
                </>
              )}
            </Button>
          )
        }
      />

      <div className="min-h-0 flex-1 overflow-y-auto">
        {creating && (
          <div className="mx-4 mt-3 space-y-3 rounded-md border border-border bg-panel px-3 py-3">
            <label className="block text-xs text-muted">
              Workspace id
              <Input
                aria-label="workspace id"
                className="mt-1"
                placeholder="e.g. acme-eu"
                value={newWs}
                onChange={(e) => setNewWs(e.target.value)}
              />
            </label>
            <label className="block text-xs text-muted">
              Display name
              <Input
                aria-label="workspace name"
                className="mt-1"
                placeholder="e.g. Acme EU (optional)"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
              />
            </label>
            <Button
              size="sm"
              aria-label="create workspace"
              disabled={!newWs.trim()}
              onClick={() => void save()}
            >
              Create workspace
            </Button>
          </div>
        )}

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
