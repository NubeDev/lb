// The API-keys admin api client — one call per export, mirroring the host `apikey` service verbs and
// the gateway `/admin/apikeys` routes 1:1 (api-keys scope). The workspace is the session's (the
// gateway derives it from the token); never passed. The raw secret is returned ONLY by
// `createApiKey`/`rotateApiKey` (one-time bearer) — `list`/`get` never carry a hash or secret.

import { invoke } from "@/lib/ipc/invoke";

/** The credential-free list view (no hash, no secret). Mirrors the Rust `ApiKeyView`. */
export interface ApiKeyView {
  id: string;
  label: string;
  kind: string;
  prefix: string;
  status: string;
  created_ts: number;
  expires_at: number;
  roles: string[];
  badge: "read-only" | "read-write" | "custom";
}

/** The full view (`apikey.get`) — the list view PLUS the resolved cap set. Still no hash/secret. */
export interface ApiKeyFull extends ApiKeyView {
  caps: string[];
}

/** The one-time reply to create/rotate: the bearer string carrying the raw secret, shown once. */
export interface CreatedApiKey {
  key: string;
}

export interface CreateApiKeyInput {
  label: string;
  kind?: string;
  role?: string;
  caps?: string[];
  expires_at?: number;
}

/** List the workspace's keys (credential-free). Mirrors `apikey.list`. */
export function listApiKeys(): Promise<ApiKeyView[]> {
  return invoke<ApiKeyView[]>("apikey_list", {});
}

/** Mint a key; returns the one-time bearer string (the ONLY egress of the secret). Mirrors
 * `apikey.create`. */
export function createApiKey(input: CreateApiKeyInput): Promise<CreatedApiKey> {
  return invoke<CreatedApiKey>("apikey_create", { ...input });
}

/** One key's full view incl. its resolved caps. Mirrors `apikey.get`. */
export function getApiKey(id: string): Promise<ApiKeyFull> {
  return invoke<ApiKeyFull>("apikey_get", { id });
}

/** Revoke a key (tombstone + instant local revoke). Mirrors `apikey.revoke`. */
export function revokeApiKey(id: string): Promise<void> {
  return invoke<void>("apikey_revoke", { id });
}

/** Rotate a key's secret; returns the one-time NEW bearer string (old secret dead). Mirrors
 * `apikey.rotate`. */
export function rotateApiKey(id: string): Promise<CreatedApiKey> {
  return invoke<CreatedApiKey>("apikey_rotate", { id });
}
