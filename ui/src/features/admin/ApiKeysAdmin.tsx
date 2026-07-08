// API Keys administration (api-keys scope) — create / list / revoke / rotate long-lived machine
// credentials. The raw secret is shown EXACTLY ONCE on create/rotate (a banner with a copy button +
// "you won't see this again"), then discarded from UI state; the list NEVER renders a hash or secret
// (the Rust test pins the wire; here we assert the rendered DOM too). Read-only/read-write is the
// badge from the key's assigned built-in role. All verbs re-check `mcp:apikey.manage:call` server-side
// — the tab's presence is display convenience only.
//
// Built on shadcn primitives (access-console consistency): the shared `Table` (sticky header) + the
// shared `AdminToolbar` (search + "New key"), no local page header (the `AdminView` tab strip owns it),
// no raw `<table>`. Tokens only — the revoke uses the `Button` `destructive` variant.

import { useState } from "react";
import { Copy, KeyRound, Plus, RotateCw, Trash2 } from "lucide-react";

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
import { AdminToolbar } from "./AdminToolbar";
import { useApiKeys } from "./useApiKeys";

interface Props {
  ws?: string;
}

const KINDS = ["appliance", "cli", "api", "agent"] as const;
const ROLES = ["apikey-read", "apikey-write"] as const;

export function ApiKeysAdmin(_: Props) {
  const { keys, error, newSecret, create, revoke, rotate, clearSecret } = useApiKeys();
  const [creating, setCreating] = useState(false);
  const [label, setLabel] = useState("");
  const [kind, setKind] = useState<string>("appliance");
  const [role, setRole] = useState<string>("apikey-read");
  const [filter, setFilter] = useState("");

  async function save() {
    const trimmed = label.trim();
    if (!trimmed) return;
    await create({ label: trimmed, kind, role });
    setLabel("");
    setCreating(false);
  }

  const q = filter.toLowerCase();
  const visible = keys.filter(
    (k) => k.label.toLowerCase().includes(q) || k.prefix.toLowerCase().includes(q),
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

      <AdminToolbar
        search={filter}
        onSearch={setFilter}
        searchPlaceholder="Filter keys…"
        action={
          <Button
            variant={creating ? "outline" : "default"}
            size="sm"
            aria-label="new key"
            onClick={() => setCreating((c) => !c)}
          >
            {creating ? "Cancel" : (
              <>
                <Plus size={13} /> New key
              </>
            )}
          </Button>
        }
      />

      <div className="min-h-0 flex-1 overflow-y-auto">
        {newSecret && (
          <div
            role="alert"
            className="mx-4 mt-3 space-y-2 rounded-md border border-accent/25 bg-accent/10 px-3 py-2"
          >
            <p className="text-xs font-medium text-accent">
              Copy the key now — you won&apos;t see this secret again.
            </p>
            <code className="block break-all rounded-md bg-bg px-2 py-1 font-mono text-xs">
              {newSecret}
            </code>
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                aria-label="copy secret"
                onClick={() => void navigator.clipboard?.writeText(newSecret)}
              >
                <Copy size={12} /> Copy
              </Button>
              <Button variant="ghost" size="sm" aria-label="dismiss secret" onClick={clearSecret}>
                Dismiss
              </Button>
            </div>
          </div>
        )}

        {creating && (
          <div className="mx-4 mt-3 space-y-3 rounded-md border border-border bg-panel px-3 py-3">
            <label className="block text-xs text-muted">
              Label
              <Input
                aria-label="key label"
                className="mt-1"
                placeholder="e.g. rooftop-hvac"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
              />
            </label>
            <fieldset className="text-xs text-muted">
              <legend className="mb-1">Kind</legend>
              <div className="flex flex-wrap gap-1">
                {KINDS.map((k) => (
                  <Button
                    key={k}
                    variant={kind === k ? "default" : "outline"}
                    size="sm"
                    aria-label={`kind ${k}`}
                    aria-pressed={kind === k}
                    onClick={() => setKind(k)}
                  >
                    {k}
                  </Button>
                ))}
              </div>
            </fieldset>
            <fieldset className="text-xs text-muted">
              <legend className="mb-1">Role</legend>
              <div className="flex flex-wrap gap-1">
                {ROLES.map((r) => (
                  <Button
                    key={r}
                    variant={role === r ? "default" : "outline"}
                    size="sm"
                    aria-label={`role ${r}`}
                    aria-pressed={role === r}
                    onClick={() => setRole(r)}
                  >
                    {r}
                  </Button>
                ))}
              </div>
            </fieldset>
            <Button
              size="sm"
              aria-label="create key"
              disabled={!label.trim()}
              onClick={() => void save()}
            >
              Create key
            </Button>
          </div>
        )}

        {visible.length === 0 ? (
          <AppEmptyState
            icon={KeyRound}
            title={filter ? "No keys match." : "No API keys yet."}
            description={filter ? "Clear the filter to see every key." : "Create one to mint a credential."}
          />
        ) : (
          <Table aria-label="api keys">
            <TableHeader sticky>
              <TableRow>
                <TableHead>Label</TableHead>
                <TableHead>Kind</TableHead>
                <TableHead>Prefix</TableHead>
                <TableHead>Badge</TableHead>
                <TableHead>Status</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {visible.map((k) => (
                <TableRow key={k.id}>
                  <TableCell className="font-medium text-fg">{k.label}</TableCell>
                  <TableCell className="text-muted">{k.kind}</TableCell>
                  <TableCell className="font-mono text-muted">{k.prefix}</TableCell>
                  <TableCell>{k.badge}</TableCell>
                  <TableCell>{k.status === "__revoked__" ? "revoked" : k.status}</TableCell>
                  <TableCell className="text-right">
                    {k.status !== "__revoked__" && (
                      <span className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`rotate key ${k.id}`}
                          onClick={() => void rotate(k.id)}
                        >
                          <RotateCw size={13} /> Rotate
                        </Button>
                        <Button
                          variant="destructive"
                          size="sm"
                          aria-label={`revoke key ${k.id}`}
                          onClick={() => void revoke(k.id)}
                        >
                          <Trash2 size={13} /> Revoke
                        </Button>
                      </span>
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>
    </div>
  );
}
