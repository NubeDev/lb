/**
 * The webhook helper — the **third-party caller path**. A service the admin has
 * shared a webhook secret with signs the raw body and POSTs to
 * `/hooks/{ws}/{id}`. The gateway verifies the HMAC over the **exact received
 * bytes** (see `rust/role/gateway/src/routes/webhook.rs`), so this helper takes
 * `Uint8Array`, never a string — HMAC over a re-serialized body is the single
 * most common webhook-integration bug.
 *
 * Uses Node's `node:crypto` for HMAC (the only place the lib needs a node
 * builtin — browsers would use `crypto.subtle`, which is a follow-up).
 */

import { createHmac } from "node:crypto";
import { ApiError } from "./client.js";
import type { Client } from "./client.js";

/** `POST /hooks/{ws}/{id}` reply (see `routes/webhook.rs::Accepted`). */
export interface WebhookAccepted {
  id: string;
  series: string;
  seq: number;
}

/** Sign `body` with `secret` (the shared secret the admin got at webhook
 * create). Returns the value to send in the admin-picked header (default
 * `X-Signature`), formatted as `sha256=<64 hex>` — exactly what the gateway's
 * `signature` mode expects.
 *
 * **Body must be the raw bytes you POST** — sign-then-reformat breaks the
 * signature. */
export function signWebhook(secret: Uint8Array | string, body: Uint8Array | string): string {
  const mac = createHmac("sha256", typeof secret === "string" ? Buffer.from(secret) : secret);
  mac.update(typeof body === "string" ? Buffer.from(body) : body);
  return `sha256=${mac.digest("hex")}`;
}

/** `POST /hooks/{ws}/{id}` with caller-supplied headers. For `signature` mode,
 * pass `{ "X-Signature": signWebhook(secret, body) }` (or the admin-picked
 * header name). For `bearer` mode, pass `{ Authorization: "Bearer lbk_…" }`.
 * The `Client`'s own bearer is NOT applied here — the inbound webhook route is
 * the one gateway route that takes no session token. */
export async function postWebhook(
  client: Client,
  ws: string,
  id: string,
  headers: Record<string, string>,
  body: Uint8Array | string,
): Promise<WebhookAccepted> {
  const path = `/hooks/${encodeURIComponent(ws)}/${encodeURIComponent(id)}`;
  const buf = typeof body === "string" ? Buffer.from(body) : body;
  // Bypass `Client.request` (which would attach the bearer); build the fetch
  // directly so the only credential on the wire is the one the caller passed.
  const url = `${client.baseUrl}${path}`;
  const resp = await fetch(url, {
    method: "POST",
    headers: { accept: "application/json", "content-type": "application/json", ...headers },
    body: buf as BodyInit,
  });
  const text = await resp.text();
  if (!resp.ok) {
    throw new ApiError(resp.status, text, path);
  }
  return JSON.parse(text) as WebhookAccepted;
}
