// The FROZEN host contract this remote is built against. Mirrors the shell's
// `ext-host/federation.ts` (RemoteMount) + `ext-host/bridge.ts` (ExtBridge) byte-for-byte so
// the in-process mount handshake type-checks against what the shell actually passes. Copied
// from proof-panel (the packaging precedent) — the same seam, a different consumer.

/** Page context the shell hands to `mountPage` — the active workspace (the hard tenant wall). */
export interface MountCtx {
  workspace: string;
}

/** The page bridge: the ONLY data seam. `call` forwards a granted MCP tool; the host re-checks
 *  every call, workspace-first. The page holds no token, DB, or fetch. */
export interface Bridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
}

/** The widget bridge the dashboard passes to `mountWidget`. Adds an OPTIONAL `watch` for live
 *  streams (the series SSE). `watch` is optional because the frozen PAGE bridge is call-only
 *  (verified in ui/src/features/ext-host/bridge.ts) — the ValueSource falls back to polling
 *  `series.latest` when `watch` is absent (parent scope Open question 2: polling is fine for
 *  phases 1–2). Mirrors proof-panel's `WidgetBridge`. */
export interface WidgetBridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  watch?: (
    tool: string,
    args: Record<string, unknown>,
    onEvent: (e: unknown) => void,
  ) => () => void;
}

/** The widget mount context (mirrors proof-panel): the workspace wall + the cell's binding/options
 *  (the scene id the cell renders lands here). */
export interface WidgetCtx {
  workspace: string;
  binding: Record<string, unknown>;
  options: Record<string, unknown>;
}
