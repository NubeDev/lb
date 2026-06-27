// The FROZEN host contract this remote is built against. These types mirror the shell's
// `ext-host/federation.ts` (RemoteMount) and `ext-host/bridge.ts` (ExtBridge) byte-for-byte so the
// in-process mount handshake type-checks against what the shell actually passes.

/** Page context the shell hands to `mount` — the active workspace (the hard tenant wall). */
export interface MountCtx {
  workspace: string;
}

/** The ONLY data seam. `call` forwards a granted read-only MCP tool; the host re-checks every call. */
export interface Bridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
}
