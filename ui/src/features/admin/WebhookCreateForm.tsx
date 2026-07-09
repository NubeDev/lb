// The create-webhook form (webhooks scope) — revealed inline below the header when the user
// clicks "New webhook" (the Rules-view pattern of a form revealed in the reading flow, not a
// modal). Holds the name + the auth-mode picker (bearer|signature) + the signature header field
// (signature mode only). The parent owns whether the form is open; this component owns the
// in-progress draft and calls `onCreate` with the validated input. One component per file
// (FILE-LAYOUT).

import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { CreateWebhookInput, WebhookAuthMode } from "@/lib/admin/webhooks.api";

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

interface Props {
  onCreate: (input: CreateWebhookInput) => Promise<void>;
  onCancel: () => void;
}

export function WebhookCreateForm({ onCreate, onCancel }: Props) {
  const [name, setName] = useState("");
  // `signature` is the safer default — a third-party caller can't accidentally send a bearer.
  const [mode, setMode] = useState<WebhookAuthMode>("signature");
  const [hmacHeader, setHmacHeader] = useState<string>("X-Signature");

  async function save() {
    const trimmed = name.trim();
    if (!trimmed) return;
    await onCreate({
      name: trimmed,
      auth_mode: mode,
      hmac_header: mode === "signature" ? hmacHeader.trim() || "X-Signature" : undefined,
    });
  }

  return (
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
              variant={mode === m.key ? "default" : "outline"}
              size="sm"
              aria-label={`mode ${m.key}`}
              aria-pressed={mode === m.key}
              onClick={() => setMode(m.key)}
            >
              {m.label}
            </Button>
          ))}
        </div>
        <p className="mt-1 text-[11px] text-muted">{MODES.find((m) => m.key === mode)?.hint}</p>
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
      <div className="flex gap-2">
        <Button
          variant="default"
          size="sm"
          aria-label="create webhook"
          disabled={!name.trim()}
          onClick={() => void save()}
        >
          Create webhook
        </Button>
        <Button variant="ghost" size="sm" aria-label="cancel create webhook" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
