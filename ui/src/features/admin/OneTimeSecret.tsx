// The one-time credential banner shown after create/rotate (webhooks scope). Mode-aware copy:
// `bearer` shows the `lbk_…` string + the `Authorization: Bearer …` hint; `signature` shows the
// shared secret + the header the caller must HMAC-sign with. The secret is shown EXACTLY ONCE
// — the parent discards it from UI state on dismiss; the roster NEVER renders it. The banner
// carries `role="alert"` so the gateway test (and a screen reader) lands on it the moment it
// appears. One component per file (FILE-LAYOUT).

import { Copy } from "lucide-react";

import { Button } from "@/components/ui/button";
import { gatewayUrl } from "@/lib/ipc/http";
import type { CreatedWebhook } from "@/lib/admin/webhooks.api";

interface Props {
  created: CreatedWebhook;
  onDismiss: () => void;
}

/** Compose the public inbound URL from the gateway origin + the hook's url_path. Empty origin
 *  (no window, e.g. jsdom) → just the path, so tests render the path cleanly without a host. */
function publicUrl(urlPath: string): string {
  const origin = gatewayUrl();
  return origin ? `${origin}${urlPath}` : urlPath;
}

export function OneTimeSecret({ created, onDismiss }: Props) {
  const hmacHeader = created.hmac_header || "X-Signature";
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
            : `Shared secret (HMAC-SHA256-sign the raw body, send sha256=<hex> in ${hmacHeader})`}
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
