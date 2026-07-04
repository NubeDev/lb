// The session — the verified identity the app holds after login. Mirrors the gateway `LoginReply`
// and `ui/src/lib/session/session.types.ts` one-to-one. The app only ever holds issued tokens —
// never the signing key; the workspace always comes from the signed token, never client state.

export interface Session {
  /** The signed bearer token the gateway issued — sent on every request. */
  token: string;
  /** The logged-in principal (`user:…`). */
  principal: string;
  /** The current workspace (from the token) — the hard wall every verb scopes to. */
  workspace: string;
  /** The capabilities the token carries — read to decide what to *show* only; the gateway
   *  re-checks every verb server-side. */
  caps?: string[];
}

/** Everything the app persists about identity: one session per workspace + the active pointer.
 *  Multi-workspace = token per workspace (app-shell scope); switching activates another token. */
export interface StoredSessions {
  /** The active workspace id — must be a key of `sessions`. */
  active: string;
  /** One issued session per workspace the user has logged into. */
  sessions: Record<string, Session>;
}
