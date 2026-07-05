// Webhooks administration (webhooks scope) — create / list / revoke / rotate inbound-HTTP
// endpoints. Each webhook has a stable public URL `POST /hooks/{ws}/{id}` and one of two auth
// modes (admin-selected per hook):
//   - `bearer`: the caller sends `Authorization: Bearer lbk_{ws}.{keyid}.{secret}` (the credential
//     IS a real apikey — reuses the apikey model verbatim, scoped to this hook).
//   - `signature`: the caller HMAC-SHA256-signs the raw body with a shared secret and sends
//     `sha256=<hex>` in an admin-picked header (default `X-Signature`).
//
// The raw secret is shown EXACTLY ONCE on create/rotate (the OneTimeSecret banner with a copy
// button + "you won't see this again"), then discarded from UI state; the roster NEVER renders a
// hash, secret, `bearer_key_id`, or `secret_ref`. All verbs re-check `mcp:webhook.manage:call`
// server-side — the page's presence is display convenience only. Reached from the sidebar's Data
// group (beside Datasources/Ingest), NOT the AdminView tabs.
//
// Wraps in the canonical `AppPage` shell so the page reads like the Dashboards/Rules surfaces
// (accent header, workspace chip, settings link, page-transition Reveal) — the same shell every
// full-screen surface obeys (ui-standards-scope rule 2). This file is the page + wiring only;
// the wizard step, the roster, and the one-time banner each live in their own file (FILE-LAYOUT).

import { useState } from "react";
import { Webhook } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { AppEmptyState } from "@/components/app/empty-state";
import { Button } from "@/components/ui/button";
import { useWebhooks } from "./useWebhooks";
import { OneTimeSecret } from "./OneTimeSecret";
import { WebhookCreateForm } from "./WebhookCreateForm";
import { WebhookRoster } from "./WebhookRoster";

interface Props {
  ws: string;
}

export function WebhooksAdmin({ ws }: Props) {
  const { webhooks, error, newSecret, create, revoke, rotate, clearSecret } = useWebhooks();
  const [creating, setCreating] = useState(false);

  const actions = (
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
    <AppPage
      label="webhooks admin"
      icon={Webhook}
      title="Webhooks"
      description="Receive inbound HTTP as ingest samples."
      workspace={ws}
      error={error}
      actions={actions}
    >
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
        {newSecret && <OneTimeSecret created={newSecret} onDismiss={clearSecret} />}

        {creating && (
          <WebhookCreateForm
            onCreate={async (input) => {
              await create(input);
              setCreating(false);
            }}
            onCancel={() => setCreating(false)}
          />
        )}

        {webhooks.length === 0 && !creating ? (
          <AppEmptyState
            icon={Webhook}
            title="No webhooks yet."
            description="Create one to expose a stable inbound URL that lands hits as ingest samples."
          />
        ) : (
          <WebhookRoster webhooks={webhooks} onRotate={rotate} onRevoke={revoke} />
        )}
      </div>
    </AppPage>
  );
}
