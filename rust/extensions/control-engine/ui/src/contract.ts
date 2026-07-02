// The FROZEN host contract this remote is built against (mirrors proof-panel's `app/contract.ts`,
// which itself mirrors the shell's `ext-host/federation.ts` + `ext-host/bridge.ts` byte-for-byte). The
// one addition over proof-panel's page bridge is the OPTIONAL `watch` — the live half the shell wires
// over the series SSE (absent under Tauri/tests with no gateway; the transport degrades gracefully).

/** Page context the shell hands to `mount` — the active workspace (the hard tenant wall). */
export interface MountCtx {
  workspace: string;
}

/** The ONLY data seam the page reaches the platform through. Every CE canvas action becomes a caps-gated
 *  `call`; the host re-checks each call against `install-scope ∩ caller-grant` (a read-only user's canvas
 *  is read-only because the write tools fall outside their grant → the call rejects). `watch` maps a
 *  `series.watch` onto the shipped series SSE for the live COV feed; it is OPTIONAL — the shell omits it
 *  where there is no SSE transport (Tauri desktop, the vitest harness), and the transport degrades to a
 *  static (no live updates) canvas rather than throwing. */
export interface Bridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  watch?: (
    tool: string,
    args: Record<string, unknown>,
    onEvent: (e: unknown) => void,
  ) => () => void;
}
