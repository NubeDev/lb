// Shell i18n — every user-facing string flows through these catalogs (release scope, gap d),
// via the @nube/ext-ui-sdk seam (one mechanism, never forked). Locale resolution is the pinned
// chain: the member's `language` pref (fetched post-login via `prefs.resolve`) →
// `navigator.language` base → `en`. Catalogs hold the words; components hold keys.

import { createContext, useContext, useEffect, useState } from "react";
import type { ReactNode } from "react";
import {
  makeTranslator,
  resolveLocale,
  type Catalogs,
  type Translator,
} from "@nube/ext-ui-sdk";
import { mcpCall, type Session } from "./ipc";

export const ENABLED_LOCALES = ["en", "es"];

// Key-parity between these two maps is CI-gated (i18n.test.ts → catalogParity).
export const CATALOGS: Catalogs = {
  en: {
    "login.title": "Sign in",
    "login.user": "user",
    "login.workspace": "workspace",
    "login.password": "password",
    "login.failed": "login failed",
    "ext.none": "No extension UI available.",
    "ext.list_failed": "ext.list failed",
    "ext.loading": "Loading…",
    "ext.mount_failed": "mount failed",
    "shell.sign_out": "Sign out",
  },
  es: {
    "login.title": "Iniciar sesión",
    "login.user": "usuario",
    "login.workspace": "espacio de trabajo",
    "login.password": "contraseña",
    "login.failed": "error al iniciar sesión",
    "ext.none": "No hay interfaz de extensión disponible.",
    "ext.list_failed": "falló ext.list",
    "ext.loading": "Cargando…",
    "ext.mount_failed": "falló el montaje",
    "shell.sign_out": "Cerrar sesión",
  },
};

const I18nContext = createContext<Translator>(makeTranslator(CATALOGS, "en"));

/** The shell's translate hook — components call `const t = useT()` and render keys. */
export function useT(): Translator {
  return useContext(I18nContext);
}

/**
 * Provides the translator for the resolved locale. Pre-auth: browser language → en. Post-login:
 * best-effort `prefs.resolve` fetches the member's `language` pref (an invited member has it
 * seeded from the invite's locale); failure falls back to the pre-auth chain — never blocks.
 */
export function I18nProvider({ session, children }: { session: Session | null; children: ReactNode }) {
  const [locale, setLocale] = useState(() => resolveLocale(null, ENABLED_LOCALES));

  useEffect(() => {
    let cancelled = false;
    if (!session) {
      setLocale(resolveLocale(null, ENABLED_LOCALES));
      return;
    }
    (async () => {
      try {
        const resolved: any = await mcpCall("prefs.resolve", {});
        const lang = resolved?.language ?? resolved?.resolved?.language;
        if (!cancelled) setLocale(resolveLocale(lang, ENABLED_LOCALES));
      } catch {
        // Pref fetch is best-effort — keep the browser-language locale.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [session?.token, session?.workspace]);

  return <I18nContext.Provider value={makeTranslator(CATALOGS, locale)}>{children}</I18nContext.Provider>;
}
