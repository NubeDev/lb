// Shared helper for the real-gateway tests: point `invoke` at the spawned server and log in for a
// real signed session. Importing this from a `*.gateway.test.tsx` gives a `signInReal(workspace)`
// that mints a real token (carrying the dev claim set — which includes the data-console caps) and
// stores it, so every subsequent `invoke` call hits the real backend with a real bearer token.

import { inject, vi } from "vitest";

import { login } from "@/lib/session/session.api";
import { setSession, sessionToken } from "@/lib/session/session.store";

/** Make `invoke` take the real HTTP path to the spawned gateway (stub `VITE_GATEWAY_URL` to its URL).
 *  Call once per test file (idempotent). */
export function useRealGateway(): void {
  vi.stubEnv("VITE_GATEWAY_URL", inject("gatewayUrl"));
}

/** Log in `user` into `workspace` against the real gateway and store the session. Returns the session
 *  (token + principal + caps). A unique workspace per test keeps the shared real backend isolated. */
export async function signInReal(user: string, workspace: string) {
  const session = await login(user, workspace);
  setSession(session);
  return session;
}

/** POST to a test-only `/_seed/*` route on the spawned gateway, authenticated by the current session
 *  token (so the seed lands in the session's workspace — the real write path, behind the workspace
 *  wall). For surfaces with no public create route (inbox item / outbox effect / extension install). */
async function seed(
  kind: "inbox" | "outbox" | "extension" | "series",
  body: unknown,
): Promise<void> {
  const url = inject("gatewayUrl");
  const token = sessionToken();
  const res = await fetch(`${url}/_seed/${kind}`, {
    method: "POST",
    headers: { "content-type": "application/json", authorization: `Bearer ${token}` },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error(`seed ${kind} failed: ${res.status} ${await res.text()}`);
}

/** Seed a real durable inbox item into the session workspace. */
export function seedInbox(item: {
  id: string;
  channel: string;
  author: string;
  body: string;
  ts: number;
  meta?: Record<string, unknown>;
}): Promise<void> {
  return seed("inbox", item);
}

/** Seed a real outbox effect into the session workspace. */
export function seedOutbox(effect: {
  id: string;
  target: string;
  action: string;
  payload?: string;
  idempotency_key?: string;
  status?: string;
  attempts?: number;
  max_attempts?: number;
  next_attempt_ts?: number;
  ts: number;
}): Promise<void> {
  return seed("outbox", {
    effect: {
      payload: "",
      idempotency_key: effect.id,
      status: "pending",
      attempts: 0,
      max_attempts: 5,
      next_attempt_ts: 0,
      ...effect,
    },
  });
}

/** Seed ONE discoverable series into the session workspace through the REAL write path: a committed
 *  sample value (so `series.latest` reads it) + a `key:value` tag edge on the `series:<name>` entity
 *  (so `series.find` discovers it). Used by the proof-panel real-gateway test — never a fake row. */
export function seedSeries(s: {
  series: string;
  seq: number;
  payload: unknown;
  key: string;
  value: unknown;
}): Promise<void> {
  return seed("series", s);
}

/** Seed a real extension install into the session workspace. */
export function seedExtension(ext: {
  ext: string;
  version: string;
  tier?: "wasm" | "native";
  enabled?: boolean;
  ui?: { entry: string; label: string; icon?: string; scope?: string[] };
  widgets?: { entry: string; label: string; icon?: string; scope?: string[] }[];
}): Promise<void> {
  return seed("extension", ext);
}
