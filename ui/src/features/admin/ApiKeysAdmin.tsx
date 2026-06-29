// API Keys administration (api-keys scope) — create / list / revoke / rotate long-lived machine
// credentials. The raw secret is shown EXACTLY ONCE on create/rotate (a banner with a copy button +
// "you won't see this again"), then discarded from UI state; the list NEVER renders a hash or secret
// (the Rust test pins the wire; here we assert the rendered DOM too). Read-only/read-write is the
// badge from the key's assigned built-in role. All verbs re-check `mcp:apikey.manage:call` server-side
// — the tab's presence is display convenience only. Uses the shadcn primitives (ui-standards scope).

import { useState } from "react";
import { Copy, KeyRound, RotateCw, Trash2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { AdminPanel } from "./AdminPanel";
import { useApiKeys } from "./useApiKeys";

interface Props {
  ws: string;
}

const KINDS = ["appliance", "cli", "api", "agent"] as const;
const ROLES = ["apikey-read", "apikey-write"] as const;

export function ApiKeysAdmin({ ws }: Props) {
  const { keys, error, newSecret, create, revoke, rotate, clearSecret } = useApiKeys();
  const [creating, setCreating] = useState(false);
  const [label, setLabel] = useState("");
  const [kind, setKind] = useState<string>("appliance");
  const [role, setRole] = useState<string>("apikey-read");

  async function save() {
    const trimmed = label.trim();
    if (!trimmed) return;
    await create({ label: trimmed, kind, role });
    setLabel("");
    setCreating(false);
  }

  const action = (
    <Button variant="default" size="sm" aria-label="new key" onClick={() => setCreating((c) => !c)}>
      <KeyRound size={13} /> New key
    </Button>
  );

  return (
    <AdminPanel icon={KeyRound} title="API Keys" ws={ws} action={action} error={error}>
      <div className="space-y-3 px-4 py-3">
        {newSecret && (
          <div role="alert" className="space-y-2 rounded border border-accent/25 bg-accent/10 px-3 py-2">
            <p className="text-xs font-medium text-accent">
              Copy the key now — you won&apos;t see this secret again.
            </p>
            <code className="block break-all rounded bg-bg px-2 py-1 font-mono text-xs">{newSecret}</code>
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
          <div className="space-y-3 rounded border border-border bg-panel px-3 py-3">
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
                    variant={kind === k ? "solid" : "outline"}
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
                    variant={role === r ? "solid" : "outline"}
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
            <Button variant="default" size="sm" aria-label="create key" disabled={!label.trim()} onClick={() => void save()}>
              Create key
            </Button>
          </div>
        )}

        {keys.length === 0 ? (
          <p className="text-sm text-muted">No API keys yet. Create one to mint a credential.</p>
        ) : (
          <table className="w-full text-sm" aria-label="api keys">
            <thead>
              <tr className="border-b border-border text-left text-xs text-muted">
                <th className="px-2 py-1.5 font-medium">Label</th>
                <th className="px-2 py-1.5 font-medium">Kind</th>
                <th className="px-2 py-1.5 font-medium">Prefix</th>
                <th className="px-2 py-1.5 font-medium">Badge</th>
                <th className="px-2 py-1.5 font-medium">Status</th>
                <th className="px-2 py-1.5 font-medium" />
              </tr>
            </thead>
            <tbody>
              {keys.map((k) => (
                <tr key={k.id} className="border-b border-border/50">
                  <td className="px-2 py-1.5">{k.label}</td>
                  <td className="px-2 py-1.5 text-xs text-muted">{k.kind}</td>
                  <td className="px-2 py-1.5 font-mono text-xs text-muted">{k.prefix}</td>
                  <td className="px-2 py-1.5 text-xs">{k.badge}</td>
                  <td className="px-2 py-1.5 text-xs">
                    {k.status === "__revoked__" ? "revoked" : k.status}
                  </td>
                  <td className="px-2 py-1.5 text-right">
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
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </AdminPanel>
  );
}
