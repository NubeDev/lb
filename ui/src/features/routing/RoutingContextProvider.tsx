import { createContext, useContext, type ReactNode } from "react";

import type { RoutingContext } from "./context";

const AppRoutingContext = createContext<RoutingContext | null>(null);

export function RoutingContextProvider({
  value,
  children,
}: {
  value: RoutingContext;
  children: ReactNode;
}) {
  return <AppRoutingContext.Provider value={value}>{children}</AppRoutingContext.Provider>;
}

export function useAppRoutingContext(): RoutingContext {
  const ctx = useContext(AppRoutingContext);
  if (!ctx) throw new Error("routing context missing");
  return ctx;
}
