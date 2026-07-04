// The ONE GatewayClient the shell holds — everything reaches the node through it (the invoke seam;
// the app adds no verbs, no caps). Rebuilt when the node URL changes; the keychain storage rides
// along so a rebuilt client restores the same sessions.

import { createGatewayClient, type GatewayClient } from '@nube/app-sdk';
import { keychainSessionStorage } from '../features/session/keychain.storage';
import { nodeUrl, subscribeNodeUrl } from './node-url.store';

let client: GatewayClient | null = null;
let builtFor = '';
const listeners = new Set<() => void>();

subscribeNodeUrl(() => {
  client = null; // lazily rebuilt on next read
  for (const l of listeners) l();
});

/** The client for the current node URL, or null before one is configured. */
export function gatewayClient(): GatewayClient | null {
  const url = nodeUrl();
  if (!url) return null;
  if (!client || builtFor !== url) {
    client = createGatewayClient({ baseUrl: url, storage: keychainSessionStorage() });
    builtFor = url;
    void client.session.restore();
  }
  return client;
}

export function subscribeClient(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
