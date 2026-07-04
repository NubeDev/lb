// Compose the whole app client for one gateway: session store + `invoke` + live streams, wired so
// every request reads the ACTIVE workspace's token and a 401 drops that session (fall back to
// login). This is the single object the RN shell (and the gateway tests) construct.

import type { GatewayConfig } from "./config";
import { createInvoke, type Invoke } from "./invoke";
import { openChannelStream, type ChannelStreamHandlers } from "../sse/channel.stream";
import type { SseStream } from "../sse/stream";
import { createSessionStore, type SessionStore } from "../session/session.store";
import type { SessionStorage } from "../session/session.storage";
import type { Session } from "../session/session.types";

export interface GatewayClientOptions {
  /** The gateway base URL, e.g. `http://192.168.1.10:8080`. */
  baseUrl: string;
  /** Where sessions persist — keychain on device, memory in tests. */
  storage: SessionStorage;
  fetchImpl?: typeof fetch;
}

export interface GatewayClient {
  invoke: Invoke;
  session: SessionStore;
  /** Log `user` into `workspace` (dev-login `POST /login`), store + activate the session. */
  login(user: string, workspace: string): Promise<Session>;
  /** Switch the active workspace. Uses the stored token if one exists; otherwise re-mints by
   *  logging the same user into `ws` (there is no server re-mint route yet — global-identity
   *  scope; re-login IS the re-mint for the dev credential). */
  switchWorkspace(ws: string): Promise<Session>;
  /** Open the live SSE feed for a channel in the active workspace. */
  streamChannel(channel: string, handlers: ChannelStreamHandlers): SseStream;
}

export function createGatewayClient(opts: GatewayClientOptions): GatewayClient {
  const session = createSessionStore(opts.storage);
  const config: GatewayConfig = {
    baseUrl: opts.baseUrl,
    fetchImpl: opts.fetchImpl,
    getToken: () => session.token(),
    // The stored token no longer verifies — drop THAT workspace's session only.
    onAuthError: () => {
      const ws = session.current()?.workspace;
      if (ws) session.logout(ws);
    },
  };
  const invoke = createInvoke(config);

  return {
    invoke,
    session,
    async login(user, workspace) {
      const issued = await invoke<Session>("login", { user, workspace });
      session.activate(issued);
      return issued;
    },
    async switchWorkspace(ws) {
      const stored = session.current();
      if (session.workspaces().includes(ws)) {
        session.switchTo(ws);
        return session.current() as Session;
      }
      if (!stored) throw new Error("not logged in");
      // Re-mint by re-login: same user, new workspace (membership is checked server-side).
      return this.login(stored.principal.replace(/^user:/, ""), ws);
    },
    streamChannel(channel, handlers) {
      return openChannelStream(config, channel, handlers);
    },
  };
}
