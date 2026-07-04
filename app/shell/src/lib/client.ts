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
    client = createGatewayClient({ baseUrl: url, storage: keychainSessionStorage() });
    void client.session.restore();
  }
  return client;
}
