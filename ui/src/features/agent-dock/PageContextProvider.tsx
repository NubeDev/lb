// The PageContextProvider (agent-dock scope, resolved decision 3) — the shell-level seam that exposes
// "where the user is" as a live callback. v1 defaults to the ROUTER-derived context (buildPageContext
// over the current location); a later feature may override it (active panel, focused cell) by wrapping
// a nested provider. The dock reads `usePageContext()().capture()` at SEND TIME so each message
// carries the page the user was on when they hit send (ask → navigate → ask captures the new page).
//
// FILE-LAYOUT: the provider + hook only; the pure derivation lives in `pageContext.ts`.

import { createContext, useCallback, useContext, useMemo, type ReactNode } from "react";
import { useLocation } from "@tanstack/react-router";

import type { PageContext } from "@/lib/channel/payload.types";
import { buildPageContext } from "./pageContext";

/** The context source: `capture()` snapshots the CURRENT page context (called per send). Kept a
 *  function (not a value) so a message captures the page live at send time, not at provider render. */
export interface PageContextSource {
  capture: () => PageContext;
}

const Ctx = createContext<PageContextSource | null>(null);

/** Provide the router-derived page context at the shell. Reads TanStack's live location + search;
 *  `capture()` builds the context on demand. A future feature override — OR a test — passes an explicit
 *  `source` to bypass the router (the seam decision 3 names); v1's shell passes none (router default). */
export function PageContextProvider({
  children,
  source,
}: {
  children: ReactNode;
  /** Override the context source (feature override / test). Omit for the router default. */
  source?: PageContextSource;
}) {
  return source ? (
    <Ctx.Provider value={source}>{children}</Ctx.Provider>
  ) : (
    <RouterPageContextProvider>{children}</RouterPageContextProvider>
  );
}

/** The v1 default: derive the context from the live TanStack location. Isolated so the router hook only
 *  runs on the default path (a test / override never invokes `useLocation`). */
function RouterPageContextProvider({ children }: { children: ReactNode }) {
  const location = useLocation();
  // `useLocation` re-renders on every navigation, so the captured pathname/search are always current.
  const pathname = location.pathname;
  const search = location.search as Record<string, unknown>;
  const capture = useCallback(() => buildPageContext(pathname, search), [pathname, search]);
  const value = useMemo<PageContextSource>(() => ({ capture }), [capture]);
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

/** Read the page-context source. Returns a stable object with `capture()`; call it at send time. A
 *  missing provider is a wiring bug (the dock is always mounted under it), so we throw. */
export function usePageContext(): PageContextSource {
  const ctx = useContext(Ctx);
  if (!ctx) throw new Error("page context provider missing");
  return ctx;
}
