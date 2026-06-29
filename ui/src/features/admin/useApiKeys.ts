// The API-keys hook — list the workspace's keys, create (capturing the one-time secret), revoke, and
// rotate (api-keys scope). One hook per file (FILE-LAYOUT). The raw secret a create/rotate returns is
// surfaced to the component exactly once via state (`newSecret`) so the UI can show it with a copy +
// "you won't see this again" warning — it is never persisted client-side.

import { useCallback, useEffect, useState } from "react";

import {
  createApiKey,
  listApiKeys,
  revokeApiKey,
  rotateApiKey,
  type ApiKeyView,
  type CreateApiKeyInput,
} from "@/lib/admin/apikeys.api";

export interface ApiKeysState {
  keys: ApiKeyView[];
  error: string | null;
  /** The one-time secret from the most recent create/rotate (cleared by the component on dismiss). */
  newSecret: string | null;
  refresh: () => Promise<void>;
  create: (input: CreateApiKeyInput) => Promise<void>;
  revoke: (id: string) => Promise<void>;
  rotate: (id: string) => Promise<void>;
  clearSecret: () => void;
}

export function useApiKeys(): ApiKeysState {
  const [keys, setKeys] = useState<ApiKeyView[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [newSecret, setNewSecret] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setKeys(await listApiKeys());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    keys,
    error,
    newSecret,
    refresh,
    create: async (input) => {
      try {
        const created = await createApiKey(input);
        setNewSecret(created.key);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    revoke: async (id) => {
      try {
        await revokeApiKey(id);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    rotate: async (id) => {
      try {
        const created = await rotateApiKey(id);
        setNewSecret(created.key);
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    clearSecret: () => setNewSecret(null),
  };
}
