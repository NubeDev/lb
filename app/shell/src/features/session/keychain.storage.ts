// The device-side SessionStorage: sessions (signed tokens) persist in the PLATFORM KEYCHAIN
// (iOS Keychain / Android Keystore via react-native-keychain) — never AsyncStorage (app-shell
// scope: tokens are credentials). One generic-password entry holds the whole StoredSessions JSON
// under a fixed service id; the sdk store folds it back on boot.

import * as Keychain from 'react-native-keychain';
import type { SessionStorage, StoredSessions } from '@nube/app-sdk';

const SERVICE = 'io.nube.lazybones.sessions';

export function keychainSessionStorage(): SessionStorage {
  return {
    async load(): Promise<StoredSessions | null> {
      const entry = await Keychain.getGenericPassword({ service: SERVICE });
      if (!entry) return null;
      try {
        return JSON.parse(entry.password) as StoredSessions;
      } catch {
        // An unreadable entry is a logout, never a crash — the user just signs in again.
        return null;
      }
    },
    async save(sessions: StoredSessions | null): Promise<void> {
      if (sessions === null) {
        await Keychain.resetGenericPassword({ service: SERVICE });
        return;
      }
      await Keychain.setGenericPassword('sessions', JSON.stringify(sessions), { service: SERVICE });
    },
  };
}
