// The device-side SessionStorage: sessions (signed tokens) persist in the PLATFORM SECURE STORE
// (iOS Keychain / Android Keystore-backed encrypted store via expo-secure-store) — never AsyncStorage
// (app-shell scope: tokens are credentials). One entry holds the whole StoredSessions JSON under a
// fixed key + keychainService; the sdk store folds it back on boot.
//
// expo-secure-store replaces react-native-keychain as the proof-of-life for the Expo bare-modules
// adoption (docs/scope/app/app-expo-scope.md). The invariant is unchanged — the token lives in the OS
// secure store, never in plaintext — so this is a like-for-like backend swap behind the same seam.
// react-native-keychain stays as a (now-unused) dependency until on-device parity is proven on both
// iOS and Android; it is then dropped in the device slice (scope open question). No code imports it.

import * as SecureStore from 'expo-secure-store';
import type { SessionStorage, StoredSessions } from '@nube/app-sdk';

// Reuse the original react-native-keychain service id so an in-place upgrade keeps existing sessions:
// on iOS the keychainService maps to the same kSecAttrService, so a token stored by the old build is
// still readable here. The key namespaces the single entry within that service.
const KEYCHAIN_SERVICE = 'io.nube.lazybones.sessions';
const KEY = 'sessions';

// Sessions must survive a device reboot before first unlock is not required, but should NOT sync to
// iCloud / other devices (a bearer token is device-bound). AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY is the
// closest match to react-native-keychain's default accessible-after-first-unlock, this-device-only.
const OPTIONS: SecureStore.SecureStoreOptions = {
  keychainService: KEYCHAIN_SERVICE,
  keychainAccessible: SecureStore.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
};

export function keychainSessionStorage(): SessionStorage {
  return {
    async load(): Promise<StoredSessions | null> {
      const raw = await SecureStore.getItemAsync(KEY, OPTIONS);
      if (!raw) return null;
      try {
        return JSON.parse(raw) as StoredSessions;
      } catch {
        // An unreadable entry is a logout, never a crash — the user just signs in again.
        return null;
      }
    },
    async save(sessions: StoredSessions | null): Promise<void> {
      if (sessions === null) {
        await SecureStore.deleteItemAsync(KEY, OPTIONS);
        return;
      }
      await SecureStore.setItemAsync(KEY, JSON.stringify(sessions), OPTIONS);
    },
  };
}
