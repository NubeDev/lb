// The node URL the app points at (cloud hub or LAN edge — symmetric nodes, rule 1). Manual entry
// in v1; QR hand-off from the web shell is a cheap follow-up (scope open question). One tiny
// observable outside React, like the session store; not a credential, so plain state is fine.

let current = '';
const listeners = new Set<() => void>();

export function nodeUrl(): string {
  return current;
}

export function setNodeUrl(url: string): void {
  current = url.replace(/\/+$/, '');
  for (const l of listeners) l();
}

export function subscribeNodeUrl(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
