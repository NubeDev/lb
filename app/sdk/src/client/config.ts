// The one configuration seam for the app's gateway client. Everything platform-specific (where the
// token lives, which fetch to use) is injected here, so the client itself is platform-free — the RN
// shell, the web, and the node-side gateway tests construct it the same way.

/** How the client reaches and authenticates against one node's gateway. */
export interface GatewayConfig {
  /** The gateway base URL, e.g. `http://192.168.1.10:8080`. No trailing slash. */
  baseUrl: string;
  /** The current session bearer token, or `""` when logged out. Read per request — never cached. */
  getToken: () => string;
  /** Called when an authenticated request comes back `401` — the stored token no longer verifies
   *  (expired, or the node re-keyed). The owner drops the session so the UI falls back to login.
   *  A `403` (capability deny) never triggers this — the caller genuinely lacks the cap. */
  onAuthError?: () => void;
  /** The fetch implementation. Defaults to the global — RN's fetch on device, undici in tests. */
  fetchImpl?: typeof fetch;
}

/** The fetch to use for `config` (the injected one, else the global). */
export function fetchOf(config: GatewayConfig): typeof fetch {
  return config.fetchImpl ?? fetch;
}
