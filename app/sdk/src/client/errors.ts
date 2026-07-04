// The typed request error the client throws. Carries the HTTP status so feature code can tell a
// capability deny (403 — render "not permitted") from an auth failure (401 — fall back to login)
// from everything else, without string-matching messages.

/** A non-OK gateway response, surfaced as data — never a crash. */
export class InvokeError extends Error {
  /** The HTTP status of the failed request. */
  readonly status: number;

  constructor(status: number, message: string) {
    super(message || `request failed (${status})`);
    this.name = "InvokeError";
    this.status = status;
  }

  /** True when this is the host's capability `Denied` — the caller lacks the cap. */
  get isDenied(): boolean {
    return this.status === 403;
  }

  /** True when the session token was missing or no longer verifies. */
  get isUnauthenticated(): boolean {
    return this.status === 401;
  }
}
