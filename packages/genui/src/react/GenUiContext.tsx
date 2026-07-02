// The GenUI render context — the bridge-shaped `call/watch` seam + resolved data, injected by the host.
// Imports NOTHING from ui/src (genui-scope "Reusable outside the dashboard"): the dashboard host wires
// this to the widget iframe bridge; the channel host (later) wires it to its own bridge. The surface and
// catalog read the bridge only through this context, so the package is transport-agnostic.

import { createContext, useContext } from "react";

/** The bridge-shaped seam the host injects — the SAME shape as the widget bridge (`makeWidgetBridge`),
 *  so the dashboard host passes its bridge straight through. `call` is host-re-checked per invocation
 *  against the cell leash; the token never enters this layer. */
export interface GenUiBridge {
  call: (tool: string, args?: Record<string, unknown>) => Promise<unknown>;
}

export interface GenUiCtx {
  bridge?: GenUiBridge;
}

const Ctx = createContext<GenUiCtx>({});

export const GenUiProvider = Ctx.Provider;

export function useGenUiBridge(): GenUiBridge | undefined {
  return useContext(Ctx).bridge;
}
