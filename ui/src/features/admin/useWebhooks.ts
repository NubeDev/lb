// The webhooks hook — list the workspace's webhooks, create (capturing the one-time secret),
// revoke, and rotate (webhooks scope). One hook per file (FILE-LAYOUT). The raw credential a
// create/rotate returns is surfaced to the component exactly once via state (`newSecret`) so the
// UI can show it with a copy + "you won't see this again" warning — it is never persisted
// client-side. The `CreatedWebhook` envelope (not just the secret string) is kept so the wizard
// can also surface the URL + the auth-mode-specific copy panel (the bearer string vs. the shared
// secret + the header name).

import { useCallback, useEffect, useState } from "react";

import {
  createWebhook,
  listWebhooks,
  revokeWebhook,
  rotateWebhook,
  type CreatedWebhook,
  type CreateWebhookInput,
  type WebhookView,
} from "@/lib/admin/webhooks.api";

export interface WebhooksState {
  webhooks: WebhookView[];
  error: string | null;
  /** The one-time credential envelope from the most recent create/rotate (cleared on dismiss). */
  newSecret: CreatedWebhook | null;
  refresh: () => Promise<void>;
  create: (input: CreateWebhookInput) => Promise<void>;
  revoke: (id: string) => Promise<void>;
  rotate: (id: string) => Promise<void>;
  clearSecret: () => void;
}

export function useWebhooks(): WebhooksState {
  const [webhooks, setWebhooks] = useState<WebhookView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [newSecret, setNewSecret] = useState<CreatedWebhook | null>(null);

  const refresh = useCallback(async () => {
    try {
      setWebhooks(await listWebhooks());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    webhooks,
    error,
    newSecret,
    refresh,
    create: async (input) => {
      try {
        const created = await createWebhook(input);
        setNewSecret(created);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    revoke: async (id) => {
      try {
        await revokeWebhook(id);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    rotate: async (id) => {
      try {
        const created = await rotateWebhook(id);
        setNewSecret(created);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    clearSecret: () => setNewSecret(null),
  };
}
