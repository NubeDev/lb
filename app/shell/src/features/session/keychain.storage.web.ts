// Web stand-in for the device keychain: sessions persist in localStorage for the PREVIEW ONLY.
// This is NOT for production web (a token in localStorage is XSS-reachable) — it exists so the
// RN-Web preview keeps you logged in across reloads. The device build uses the real keychain.

// This `.web.ts` file runs ONLY in the browser preview, so it may use DOM globals the RN tsconfig
// (no `dom` lib) otherwise omits. The file-local lib reference keeps `localStorage` typed here
// without pulling DOM types into the native build.
/// <reference lib="dom" />

import type { SessionStorage, StoredSessions } from '@nube/app-sdk';

const KEY = 'lazybones.preview.sessions';

export function keychainSessionStorage(): SessionStorage {
  return {
    load: async () => {
      const raw = localStorage.getItem(KEY);
      if (!raw) return null;
      try {
        return JSON.parse(raw) as StoredSessions;
      } catch {
        return null;
      }
    },
    save: async (sessions: StoredSessions | null) => {
      if (sessions === null) localStorage.removeItem(KEY);
      else localStorage.setItem(KEY, JSON.stringify(sessions));
    },
  };
}
