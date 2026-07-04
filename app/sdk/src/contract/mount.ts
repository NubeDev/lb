// Page mount contract — the app mirror of the web page contract
// (ui/src/features/ext-host/federation.ts). ctx/bridge semantics are identical;
// only the mount mechanics differ (React component, not mount(el)).

/** Context handed to an extension Page. Workspace comes from the session token. */
export interface MountCtx {
  workspace: string;
}

/**
 * Host-mediated tool bridge. `call` is filtered against the manifest's
 * `[app].scope` client-side and re-checked by the host per call
 * (`mcp:<tool>:call`). The remote never sees the session token.
 */
export interface Bridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
}
