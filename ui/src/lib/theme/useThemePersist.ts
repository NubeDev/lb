// The prefs-authority side of the theme provider, extracted so `ThemeProvider` stays about state +
// DOM. Two jobs:
//   1. Reconcile ONCE on mount: read the authoritative theme from prefs and, if it differs from the
//      first-paint localStorage cache, adopt it — unless the user has already made a local change this
//      session (last-writer wins, no slow prefs.resolve clobbering a fresh pick).
//   2. Persist (debounced) every subsequent local change to prefs, best-effort — a member without
//      `mcp:prefs.set:call` is denied and we stay local-only (opaque, no throw to the UI).
//
// One responsibility: theme ⇄ prefs reconciliation + debounced persistence for the provider.

import { useEffect, useRef } from "react";

import { persistTheme, readResolvedTheme } from "./theme-prefs";
import type { ThemePreference } from "./theme-options";

const PERSIST_DEBOUNCE_MS = 400;

interface Options {
  theme: ThemePreference;
  /** Whether the user has changed the theme locally this session (guards the reconcile from clobbering). */
  dirty: boolean;
  onReconciled: (fromPrefs: ThemePreference) => void;
  onHydrated: () => void;
}

export function useThemePersist({ theme, dirty, onReconciled, onHydrated }: Options) {
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;

  // 1. Reconcile once on mount.
  useEffect(() => {
    let cancelled = false;
    void readResolvedTheme()
      .then((fromPrefs) => {
        if (cancelled) return;
        // Don't overwrite a change the user made before prefs.resolve returned.
        if (fromPrefs && !dirtyRef.current) onReconciled(fromPrefs);
      })
      .catch(() => {
        // No prefs access (denied / offline) — stay on the cached/default theme.
      })
      .finally(() => {
        if (!cancelled) onHydrated();
      });
    return () => {
      cancelled = true;
    };
    // Mount-only.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 2. Debounced persist on local change.
  useEffect(() => {
    if (!dirty) return;
    const t = setTimeout(() => {
      void persistTheme(theme).catch(() => {
        // Persist denied — local-only, opaque. The cache already holds the change.
      });
    }, PERSIST_DEBOUNCE_MS);
    return () => clearTimeout(t);
  }, [theme, dirty]);
}
