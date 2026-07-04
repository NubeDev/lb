// The durable-storage seam for sessions. Tokens are credentials, so WHERE they persist is a
// platform decision the sdk cannot make: the RN shell implements this over the platform keychain
// (react-native-keychain — never AsyncStorage, app-shell scope); tests use the in-memory adapter
// (real tokens, ephemeral storage — a storage adapter, not a fake backend).

import type { StoredSessions } from "./session.types";

/** Load/save the persisted session set. `null` = logged out everywhere. */
export interface SessionStorage {
  load(): Promise<StoredSessions | null>;
  save(sessions: StoredSessions | null): Promise<void>;
}

/** An ephemeral adapter: real sessions, no persistence. For tests and previews. */
export function memorySessionStorage(): SessionStorage {
  let held: StoredSessions | null = null;
  return {
    load: () => Promise.resolve(held),
    save(next) {
      held = next;
      return Promise.resolve();
    },
  };
}
