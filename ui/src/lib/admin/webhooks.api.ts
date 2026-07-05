// The webhooks admin api client — one call per export, mirroring the host `webhook` service verbs
// and the gateway `/admin/webhooks` routes 1:1 (webhooks scope). The workspace is the session's
// (the gateway derives it from the token); never passed. The raw secret is returned ONLY by
// `createWebhook`/`rotateWebhook` (one-time credential — `lbk_…` bearer for `bearer` mode, shared
// secret for `signature` mode) — `list`/`get` never carry a hash / shared-secret / `bearer_key_id`
// / `secret_ref`.
//
// The public inbound `POST /hooks/{ws}/{id}` route is NOT in this client — it's a third-party
// caller (an external service) presenting the hook's own credential, not a workspace session
// token. This client is the ADMIN surface only.

import { invoke } from "@/lib/ipc/invoke";

/** The two admin-selected auth modes (webhooks scope). */
export type WebhookAuthMode = "bearer" | "signature";

/** The credential-free list view (no hash, no secret). Mirrors the Rust `WebhookView`. */
export interface WebhookView {
  id: string;
  name: string;
  series: string;
  auth_mode: WebhookAuthMode;
  /** The inbound URL path (`/hooks/{ws}/{id}`); the shell resolves the public origin for display. */
  url_path: string;
  status: string;
  created_ts: number;
  last_hit_at: number;
}

/**
 * The one-time reply to create/rotate. `secret` is the raw credential — `lbk_{ws}.{keyid}.{secret}`
 * for `bearer` mode (the caller sends it as `Authorization: Bearer …`), the shared secret for
 * `signature` mode (the caller uses it to HMAC-sign the raw body). `hmac_header` is non-empty only
 * in `signature` mode — the header name the caller must put the signature in.
 */
export interface CreatedWebhook {
  id: string;
  url_path: string;
  secret: string;
  auth_mode: WebhookAuthMode;
  hmac_header: string;
}

export interface CreateWebhookInput {
  name: string;
  auth_mode: WebhookAuthMode;
  /** `signature` mode only: the header name the caller signs. Default `X-Signature`. */
  hmac_header?: string;
}

/** List the workspace's webhooks (credential-free). Mirrors `webhook.list`. */
export function listWebhooks(): Promise<WebhookView[]> {
  return invoke<WebhookView[]>("webhook_list", {});
}

/** Create a webhook; returns the one-time credential envelope (the ONLY egress of the secret). */
export function createWebhook(input: CreateWebhookInput): Promise<CreatedWebhook> {
  return invoke<CreatedWebhook>("webhook_create", { ...input });
}

/** One webhook's credential-free view. Mirrors `webhook.get`. */
export function getWebhook(id: string): Promise<WebhookView> {
  return invoke<WebhookView>("webhook_get", { id });
}

/** Revoke a webhook (tombstone + linked-apikey revoke + cache-bust). Mirrors `webhook.revoke`. */
export function revokeWebhook(id: string): Promise<void> {
  return invoke<void>("webhook_revoke", { id });
}

/** Rotate a webhook's credential; returns the one-time NEW raw credential (old dead). Mirrors
 * `webhook.rotate`. */
export function rotateWebhook(id: string): Promise<CreatedWebhook> {
  return invoke<CreatedWebhook>("webhook_rotate", { id });
}
