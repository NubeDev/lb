// The FROZEN host contract this remote's PAGE is built against. These types mirror the shell's
// `ext-host/federation.ts` (RemoteMount) and `ext-host/bridge.ts` (ExtBridge) so the in-process page
// mount handshake type-checks against what the shell passes. (The chart WIDGET rides a separate frames-in
// ctx — see `chart/mountChart.ts` `ChartCtx`; the page below does not use the bridge at all.)

/** Page context the shell hands to `mount` — the active workspace (the hard tenant wall). */
export interface MountCtx {
  workspace: string;
}

/** The page data seam (unused by this extension's trivial page). `call` forwards a granted MCP tool; the
 *  host re-checks every call. Present for signature parity with the frozen `mount(el, ctx, bridge)`. */
export interface Bridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
}
