// The HTTP verbs under `invoke` — the app mirror of the helpers at the bottom of
// `ui/src/lib/ipc/http.ts`. Every request carries the session bearer token from the config; the
// gateway derives principal + workspace from it (the hard wall, §7), never from the body.

import { fetchOf, type GatewayConfig } from "./config";
import { InvokeError } from "./errors";

function authHeaders(config: GatewayConfig): Record<string, string> {
  const token = config.getToken();
  return token ? { authorization: `Bearer ${token}` } : {};
}

/** GET a route; JSON reply. */
export async function getJson<T>(config: GatewayConfig, path: string): Promise<T> {
  const res = await fetchOf(config)(`${config.baseUrl}${path}`, {
    headers: authHeaders(config),
  });
  return decode<T>(config, res);
}

/** POST a JSON body. `auth: false` only for `login` (it issues the token). A `204` resolves to
 *  undefined. */
export async function postJson<T>(
  config: GatewayConfig,
  path: string,
  body: unknown,
  auth = true,
): Promise<T> {
  const res = await fetchOf(config)(`${config.baseUrl}${path}`, {
    method: "POST",
    headers: { "content-type": "application/json", ...(auth ? authHeaders(config) : {}) },
    body: JSON.stringify(body),
  });
  return decode<T>(config, res);
}

/** Fold a response into JSON or a typed `InvokeError`. A `401` on an authenticated request fires
 *  `onAuthError` (the stored token no longer verifies); a `403` is a capability deny and is left to
 *  the caller — don't log them out for lacking a cap. */
async function decode<T>(config: GatewayConfig, res: Response): Promise<T> {
  if (!res.ok) {
    if (res.status === 401 && config.getToken()) config.onAuthError?.();
    const body = await res.text().catch(() => "");
    throw new InvokeError(res.status, body);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}
