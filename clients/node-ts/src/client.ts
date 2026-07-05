/**
 * `Client` — base URL + bearer credential + the one HTTP plumbing function the
 * other verbs share. The bearer is opaque to this library; see `index.ts` for
 * why (the gateway splits on the `lbk_` prefix in one place).
 *
 * Uses the global `fetch` (Node 18+ and any modern runtime). No HTTP SDK dep.
 */

/** A structured failure from the gateway (a non-2xx response). `isDenied()`
 * covers the opaque `401|403|404` statuses the gateway returns for missing-cap
 * / cross-workspace / unknown-record (the contract never distinguishes them). */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly body: string,
    readonly path: string,
  ) {
    super(`gateway returned ${status} at ${path}: ${body}`);
    this.name = "ApiError";
  }

  isDenied(): boolean {
    return this.status === 401 || this.status === 403 || this.status === 404;
  }
}

/** The `POST /login` reply (see `rust/role/gateway/src/routes/login.rs`). */
export interface LoginReply {
  token: string;
  principal: string;
  workspace: string;
  caps: string[];
}

/** A configured gateway client. Clone is cheap (only a string + URL). */
export class Client {
  /** Construct from a base URL (e.g. `http://127.0.0.1:8080`) and a bearer
   * credential — either an API key `lbk_{ws}.{id}.{secret}` or a JWT. Read the
   * key from an env var in real code; do not hard-code it. */
  constructor(
    public readonly baseUrl: string,
    private bearer: string,
  ) {
    this.baseUrl = baseUrl.replace(/\/+$/, "");
  }

  /** Replace the bearer (used by `login`; also useful for rotation). */
  withBearer(bearer: string): Client {
    return new Client(this.baseUrl, bearer);
  }

  /** `POST /login {user, workspace}` — the dev-login path. Use for local-dev /
   * admin scripts; for a long-lived producer, mint an API key once via the
   * admin console (or `POST /admin/apikeys`) and use {@link Client} with it.
   * Returns a NEW `Client` carrying the issued session token. */
  async login(user: string, workspace: string): Promise<{ client: Client; reply: LoginReply }> {
    const reply = await this.requestJson<LoginReply>("POST", "/login", {
      user,
      workspace,
    }, /* noBearer */ true);
    return { client: this.withBearer(reply.token), reply };
  }

  /** Begin a `fetch` for `path` under `method`. Carries the bearer (unless
   * `noBearer`, used by `/login`). Use the typed verbs in `ingest.ts` /
   * `mcp.ts` / `webhook.ts` rather than calling this directly. */
  async request(
    method: string,
    path: string,
    init: { body?: unknown; headers?: Record<string, string>; noBearer?: boolean } = {},
  ): Promise<Response> {
    const url = `${this.baseUrl}${path}`;
    const headers: Record<string, string> = { accept: "application/json", ...(init.headers ?? {}) };
    if (!init.noBearer) headers.authorization = `Bearer ${this.bearer.trim()}`;
    const hasBody = init.body !== undefined;
    if (hasBody) headers["content-type"] = "application/json";
    return fetch(url, {
      method,
      headers,
      body: hasBody ? JSON.stringify(init.body) : undefined,
    });
  }

  /** Run `request`, then either return the parsed JSON or throw `ApiError`. */
  async requestJson<T>(
    method: string,
    path: string,
    body?: unknown,
    noBearer = false,
    headers?: Record<string, string>,
  ): Promise<T> {
    const resp = await this.request(method, path, { body, noBearer, headers });
    const text = await resp.text();
    if (!resp.ok) {
      throw new ApiError(resp.status, text, path);
    }
    return text.length === 0 ? (undefined as T) : (JSON.parse(text) as T);
  }
}
