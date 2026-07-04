// The ONE GatewayClient the shell holds — everything reaches the node through it (the invoke seam;
// the app adds no verbs, no caps).
//
// Built ONCE, lazily, the first time a node URL exists, and the SAME instance is kept thereafter.
// It must NOT be rebuilt when the node URL changes: `useSession` subscribes to this client's
// session store, and discarding the client would orphan that subscription — the UI would never
// react to login (the "stuck on the login screen" bug). The login flow sets the node URL first,
// then calls `gatewayClient()`, so the URL is present at build time. Re-pointing at a different
// node mid-session (rare) is a logout + relaunch concern, not a live swap.

import { createGatewayClient, type GatewayClient } from '@nube/app-sdk';
import { keychainSessionStorage } from '../features/session/keychain.storage';
import { nodeUrl } from './node-url.store';

let client: GatewayClient | null = null;

/** The one client, or null before any node URL is set. */
export function gatewayClient(): GatewayClient | null {
  const url = nodeUrl();
  if (!url) return client; // null until the first node URL is configured
  if (!client) {
    // Preview note: the preview gateway is an in-memory `test_gateway` — every `make dev` restart
    // wipes its store AND mints a fresh signing key, so any token persisted in localStorage is dead
    // after a restart. `client.restore()` (not the raw `session.restore()`) probes the node once and
    // DROPS a session it no longer honours, so the shell falls to login instead of rendering a stale
    // empty channel list ("my channels vanished"). `onUnreachable: "drop"` extends that to a node
    // that's simply down: a throwaway preview session we can't verify is worthless. A device build
    // pointed at a durable node would pass `"keep"` to stay logged in offline.
    client = createGatewayClient({
      baseUrl: url,
      storage: keychainSessionStorage(),
      onUnreachable: "drop",
    });
    void client.restore();
  }
  return client;
}
