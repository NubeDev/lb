// The session — the verified identity the UI holds after login (collaboration scope, slice 1).
// Mirrors the gateway `LoginReply`: a signed token plus the resolved principal + workspace. The
// token is the credential sent on every request; the workspace is the hard wall (§7) every verb
// scopes to. The UI only ever holds the issued token — never the signing key.

export interface Session {
  /** The signed bearer token the gateway issued — sent on every request. */
  token: string;
  /** The logged-in principal (`user:…`). */
  principal: string;
  /** The current workspace (from the token) — the hard wall every verb scopes to. */
  workspace: string;
  /** The capabilities the token carries (gateway `LoginReply.caps`). The UI reads these to decide
   *  which admin controls to *show* — a convenience only. The gateway re-checks every verb
   *  server-side; the UI cap-gate is never the security boundary (admin-console scope). */
  caps?: string[];
}
