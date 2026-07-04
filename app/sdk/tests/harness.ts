// Shared helpers for the app-sdk real-gateway tests: construct a `GatewayClient` against the
// spawned node (memory session storage — real tokens, ephemeral persistence), and mint explicit-cap
// sessions through the harness's `/_seed/*` routes for deny tests (dev-login's broad cap set would
// be too privileged). Mirrors `ui/src/test/gateway-session.ts`.

import { inject } from "vitest";

import {
  createGatewayClient,
  memorySessionStorage,
  type GatewayClient,
  type Session,
} from "../src/index";

/** A fresh client bound to the spawned gateway. One per test (or per simulated device). */
export function realClient(): GatewayClient {
  return createGatewayClient({ baseUrl: inject("gatewayUrl"), storage: memorySessionStorage() });
}

/** Mint a REAL signed session with an explicit cap set (the `/_seed/session` harness route) and
 *  activate it on `client` — for deny tests. The token is real; only its cap set is narrowed. */
export async function signInWithCaps(
  client: GatewayClient,
  user: string,
  workspace: string,
  caps: string[],
): Promise<Session> {
  const res = await fetch(`${inject("gatewayUrl")}/_seed/session`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ user, workspace, caps }),
  });
  if (!res.ok) throw new Error(`seed session failed: ${res.status} ${await res.text()}`);
  const session = (await res.json()) as Session;
  client.session.activate(session);
  return session;
}

/** Seed a real extension install into the current session's workspace (`/_seed/extension`) —
 *  the real Install record write path, never a fake row. */
export async function seedExtension(
  client: GatewayClient,
  ext: {
    ext: string;
    version: string;
    tier?: "wasm" | "native";
    enabled?: boolean;
    ui?: { entry: string; label: string; icon?: string; scope?: string[] };
  },
): Promise<void> {
  const res = await fetch(`${inject("gatewayUrl")}/_seed/extension`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${client.session.token()}`,
    },
    body: JSON.stringify(ext),
  });
  if (!res.ok) throw new Error(`seed extension failed: ${res.status} ${await res.text()}`);
}

/** Add `sub` to the current session's workspace roster through the REAL admin route
 *  (`POST /admin/members` — global-identity scope): a bootstrapped workspace refuses a second
 *  user's login until an admin admits them (decision #4). The caller must be workspace-admin. */
export async function addMember(client: GatewayClient, sub: string): Promise<void> {
  const res = await fetch(`${inject("gatewayUrl")}/admin/members`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${client.session.token()}`,
    },
    body: JSON.stringify({ sub }),
  });
  if (!res.ok) throw new Error(`add member failed: ${res.status} ${await res.text()}`);
}

/** Wait until `predicate` returns truthy (poll, 50ms), or fail after `ms`. */
export async function until<T>(predicate: () => T | undefined | false, ms = 10_000): Promise<T> {
  const deadline = Date.now() + ms;
  for (;;) {
    const v = predicate();
    if (v) return v;
    if (Date.now() > deadline) throw new Error("condition not reached in time");
    await new Promise((r) => setTimeout(r, 50));
  }
}
