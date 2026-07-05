// Webhooks administration (webhooks scope) — create / list / revoke / rotate inbound-HTTP
// endpoints. Each webhook has a stable public URL `POST /hooks/{ws}/{id}` and one of two auth modes
// (admin-selected per hook):
//   - `bearer`: the caller sends `Authorization: Bearer lbk_{ws}.{keyid}.{secret}` (the credential
//     IS a real apikey — reuses the apikey model verbatim, scoped to this hook).
//   - `signature`: the caller HMAC-SHA256-signs the raw body with a shared secret and sends
//     `sha256=<hex>` in an admin-picked header (default `X-Signature`).
//
// The raw secret is shown EXACTLY ONCE on create/rotate (a banner with a copy button + "you won't
// see this again"), then discarded from UI state; the list NEVER renders a hash, secret,
// `bearer_key_id`, or `secret_ref` (the Rust test pins the wire; here we assert the rendered DOM
// too). All verbs re-check `mcp:webhook.manage:call` server-side — the page's presence is display
// convenience only. Reached from the sidebar's Data group (beside Datasources/Ingest), NOT the
// AdminView tabs (the wizard reads like a data-surface, not an access-control surface).

import { useState } from "react";
import { Copy, RotateCw, Trash2, Webhook } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { gatewayUrl } from "@/lib/ipc/http";
import { AdminPanel } from "./AdminPanel";
import { useWebhooks } from "./useWebhooks";
import type { WebhookAuthMode } from "@/lib/admin/webhooks.api";

interface Props {
  ws: string;
}

const MODES: ReadonlyArray<{ key: WebhookAuthMode; label: string; hint: string }> = [
  {
    key: "bearer",
    label: "Bearer",
    hint: "Caller sends Authorization: Bearer <secret> (our issued key). Simplest; for callers we control.",
  },
  {
    key: "signature",
    label: "Signature",
    hint: "Caller HMAC-SHA256-signs the raw body with a shared secret. For third-parties that sign.",
  },
];

/** Compose the public inbound URL from the gateway origin + the hook's url_path. Empty origin (no
 *  window) → just the path, so the test harness (jsdom) renders the path cleanly without a host. */
function publicUrl(urlPath: string): string {
  const origin = gatewayUrl();
  return origin ? `${origin}${urlPath}` : urlPath;
}

export function WebhooksAdmin({ ws }: Props) {
  const { webhooks, error, newSecret, create, revoke, rotate, clearSecret } = useWebhooks();
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [mode, setMode] = useState<WebhookAuthMode>("signature");
  const [hmacHeader, setHmacHeader] = useState<string>("X-Signature");

  async function save() {
    const trimmed = name.trim();
    if (!trimmed) return;
    await create({
      name: trimmed,
      auth_mode: mode,
      hmac_header: mode === "signature" ? hmacHeader.trim() || "X-Signature" : undefined,
    });
    setName("");
    setCreating(false);
  }

  const action = (
    <Button
      variant="default"
      size="sm"
      aria-label="new webhook"
      onClick={() => setCreating((c) => !c)}
    >
      <Webhook size={13} /> New webhook
    </Button>
  );

  return (
    <AdminPanel icon={Webhook} title="Webhooks" ws={ws} action={action} error={error}>
      <div className="space-y-3 px-4 py-3">
        {newSecret && (
          <OneTimeSecret
            created={newSecret}
            onDismiss={clearSecret}
          />
        )}

        {creating && (
          <div className="space-y-3 rounded-md border border-border bg-panel px-3 py-3">
            <label className="block text-xs text-muted">
              Name
              <Input
                aria-label="webhook name"
                className="mt-1"
                placeholder="e.g. plant-alerts"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <fieldset className="text-xs text-muted">
              <legend className="mb-1">Auth mode</legend>
              <div className="flex flex-wrap gap-1">
                {MODES.map((m) => (
                  <Button
                    key={m.key}
                    variant={mode === m.key ? "solid" : "outline"}
                    size="sm"
                    aria-label={`mode ${m.key}`}
                    aria-pressed={mode === m.key}
                    onClick={() => setMode(m.key)}
                  >
                    {m.label}
                  </Button>
                ))}
              </div>
              <p className="mt-1 text-[11px] text-muted">
                {MODES.find((m) => m.key === mode)?.hint}
              </p>
            </fieldset>
            {mode === "signature" && (
              <label className="block text-xs text-muted">
                Signature header
                <Input
                  aria-label="hmac header"
                  className="mt-1"
                  placeholder="X-Signature"
                  value={hmacHeader}
                  onChange={(e) => setHmacHeader(e.target.value)}
                />
                <span className="mt-1 block text-[11px] text-muted">
                  The header name the caller puts the <code>sha256=&lt;hex&gt;</code> signature in.
                </span>
              </label>
            )}
            <Button
              variant="default"
              size="sm"
              aria-label="create webhook"
              disabled={!name.trim()}
              onClick={() => void save()}
            >
              Create webhook
            </Button>
          </div>
        )}

        {webhooks.length === 0 ? (
          <p className="text-sm text-muted">
            No webhooks yet. Create one to expose a stable inbound URL.
          </p>
        ) : (
          <table className="w-full text-sm" aria-label="webhooks">
            <thead>
              <tr className="border-b border-border text-left text-xs text-muted">
                <th className="px-2 py-1.5 font-medium">Name</th>
                <th className="px-2 py-1.5 font-medium">Mode</th>
                <th className="px-2 py-1.5 font-medium">URL</th>
                <th className="px-2 py-1.5 font-medium">Status</th>
                <th className="px-2 py-1.5 font-medium" />
              </tr>
            </thead>
            <tbody>
              {webhooks.map((w) => (
                <tr key={w.id} className="border-b border-border/50">
                  <td className="px-2 py-1.5">{w.name}</td>
                  <td className="px-2 py-1.5 text-xs text-muted">{w.auth_mode}</td>
                  <td className="px-2 py-1.5">
                    <code className="break-all font-mono text-xs text-muted">
                      {publicUrl(w.url_path)}
                    </code>
                  </td>
                  <td className="px-2 py-1.5 text-xs">
                    {w.status === "__revoked__" ? "revoked" : w.status}
                  </td>
                  <td className="px-2 py-1.5 text-right">
                    {w.status !== "__revoked__" && (
                      <span className="flex justify-end gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`rotate webhook ${w.id}`}
                          onClick={() => void rotate(w.id)}
                        >
                          <RotateCw size={13} /> Rotate
                        </Button>
                        <Button
                          variant="destructive"
                          size="sm"
                          aria-label={`revoke webhook ${w.id}`}
                          onClick={() => void revoke(w.id)}
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

/** The one-time credential banner shown after create/rotate. Mode-aware copy: bearer shows the
 *  `lbk_…` string + how to send it; signature shows the shared secret + the header to sign with. */
function OneTimeSecret({
  created,
  onDismiss,
}: {
  created: { secret: string; auth_mode: WebhookAuthMode; hmac_header: string; url_path: string };
  onDismiss: () => void;
}) {
  return (
    <div
      role="alert"
      className="space-y-2 rounded-md border border-accent/25 bg-accent/10 px-3 py-2"
    >
      <p className="text-xs font-medium text-accent">
        Copy the secret now — you won&apos;t see this again.
      </p>
      <div className="space-y-1">
        <p className="text-[11px] text-muted">Inbound URL</p>
        <code className="block break-all rounded-md bg-bg px-2 py-1 font-mono text-xs">
          {publicUrl(created.url_path)}
        </code>
      </div>
      <div className="space-y-1">
        <p className="text-[11px] text-muted">
          {created.auth_mode === "bearer"
            ? "Bearer credential (send as Authorization: Bearer <secret>)"
            : `Shared secret (HMAC-SHA256-sign the raw body, send sha256=<hex> in ${created.hmac_header || "X-Signature"})`}
        </p>
        <code className="block break-all rounded-md bg-bg px-2 py-1 font-mono text-xs">
          {created.secret}
        </code>
      </div>
      <div className="flex gap-2">
        <Button
          variant="outline"
          size="sm"
          aria-label="copy secret"
          onClick={() => void navigator.clipboard?.writeText(created.secret)}
        >
          <Copy size={12} /> Copy secret
        </Button>
        <Button variant="ghost" size="sm" aria-label="dismiss secret" onClick={onDismiss}>
          Dismiss
        </Button>
      </div>
    </div>
  );
}
