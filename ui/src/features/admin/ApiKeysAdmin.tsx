// API Keys administration (api-keys scope) — create / list / revoke / rotate long-lived machine
// credentials. The raw secret is shown EXACTLY ONCE on create/rotate (a banner with a copy button +
// "you won't see this again"), then discarded from UI state; the list NEVER renders a hash or secret
// (the Rust test pins the wire; here we assert the rendered DOM too). Read-only/read-write is just
// the badge from the key's assigned built-in role. All verbs re-check `mcp:apikey.manage:call`
// server-side — the tab's presence is display convenience only.

import { useState } from "react";
import { Copy, KeyRound, RotateCw, Trash2 } from "lucide-react";

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
    <button
      aria-label="new key"
      className="flex items-center gap-1 rounded bg-accent/15 px-2 py-1 text-xs text-accent"
      onClick={() => setCreating((c) => !c)}
    >
      <KeyRound size={13} /> New key
    </button>
  );

  return (
    <AdminPanel icon={KeyRound} title="API Keys" ws={ws} action={action} error={error}>
      <div className="space-y-3 px-4 py-3">
        {newSecret && (
          <div
            role="alert"
            className="space-y-2 rounded border border-accent/25 bg-accent/10 px-3 py-2"
          >
            <p className="text-xs font-medium text-accent">
              Copy the key now — you won&apos;t see this secret again.
            </p>
            <code className="block break-all rounded bg-bg px-2 py-1 font-mono text-xs">
              {newSecret}
            </code>
            <div className="flex gap-2">
              <button
                aria-label="copy secret"
                className="flex items-center gap-1 rounded bg-panel px-2 py-1 text-xs"
                onClick={() => void navigator.clipboard?.writeText(newSecret)}
              >
                <Copy size={12} /> Copy
              </button>
              <button
                aria-label="dismiss secret"
                className="rounded bg-panel px-2 py-1 text-xs text-muted"
                onClick={clearSecret}
              >
                Dismiss
              </button>
            </div>
          </div>
        )}

        {creating && (
          <div className="space-y-2 rounded border border-border bg-panel px-3 py-2">
            <div className="grid grid-cols-2 gap-2">
              <label className="text-xs text-muted">
                Label
                <input
                  aria-label="key label"
                  className="mt-1 w-full rounded bg-bg px-2 py-1 text-sm"
                  placeholder="e.g. rooftop-hvac"
                  value={label}
                  onChange={(e) => setLabel(e.target.value)}
                />
              </label>
              <label className="text-xs text-muted">
                Kind
                <select
                  aria-label="key kind"
                  className="mt-1 w-full rounded bg-bg px-2 py-1 text-sm"
                  value={kind}
                  onChange={(e) => setKind(e.target.value)}
                >
                  {KINDS.map((k) => (
                    <option key={k} value={k}>
                      {k}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <label className="text-xs text-muted">
              Role
              <select
                aria-label="key role"
                className="mt-1 w-full rounded bg-bg px-2 py-1 text-sm"
                value={role}
                onChange={(e) => setRole(e.target.value)}
              >
                {ROLES.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
            </label>
            <button
              aria-label="create key"
              className="rounded bg-accent/15 px-3 py-1 text-xs text-accent disabled:opacity-40"
              disabled={!label.trim()}
              onClick={() => void save()}
            >
              Create key
            </button>
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
                  <td className="px-2 py-1.5 text-xs">{k.status === "__revoked__" ? "revoked" : k.status}</td>
                  <td className="px-2 py-1.5 text-right">
                    {k.status !== "__revoked__" && (
                      <>
                        <button
                          aria-label={`rotate key ${k.id}`}
                          className="mx-1 text-muted hover:text-fg"
                          title="Rotate secret"
                          onClick={() => void rotate(k.id)}
                        >
                          <RotateCw size={13} />
                        </button>
                        <button
                          aria-label={`revoke key ${k.id}`}
                          className="mx-1 text-red-400 hover:text-red-300"
                          title="Revoke"
                          onClick={() => void revoke(k.id)}
                        >
                          <Trash2 size={13} />
                        </button>
                      </>
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
