// Compose the whole app client for one gateway: session store + `invoke` + live streams, wired so
// every request reads the ACTIVE workspace's token and a 401 drops that session (fall back to
// login). This is the single object the RN shell (and the gateway tests) construct.

import type { GatewayConfig } from "./config";
import { createInvoke, type Invoke } from "./invoke";
import { openChannelStream, type ChannelStreamHandlers } from "../sse/channel.stream";
import type { SseStream } from "../sse/stream";
import { createSessionStore, type SessionStore } from "../session/session.store";
import type { SessionStorage } from "../session/session.storage";
import { probeSession } from "../session/validate";
import type { Session } from "../session/session.types";

export interface GatewayClientOptions {
  /** The gateway base URL, e.g. `http://192.168.1.10:8080`. */
  baseUrl: string;
  /** Where sessions persist — keychain on device, memory in tests. */
  storage: SessionStorage;
  fetchImpl?: typeof fetch;
  /** What to do when a restored session's node is UNREACHABLE (fetch rejects — not a 401). The
   *  preview default is `"drop"`: its node is a throwaway in-memory gateway, so an unverifiable
   *  session is worthless — fall to login. A device build against a durable node may prefer `"keep"`
   *  to stay logged in offline. A dead (401) session is always dropped, regardless of this. */
  onUnreachable?: "drop" | "keep";
}

export interface GatewayClient {
  invoke: Invoke;
  session: SessionStore;
  /** Rehydrate the persisted session AND prove it still verifies against the node. Rehydrating alone
   *  (`session.restore()`) can leave a DEAD token active — the node may have re-keyed or forgotten it
   *  (an in-memory preview node does on every restart) — which the UI then renders as an empty state
   *  instead of a login prompt. This probes once (`GET /workspaces`) and drops a session the node
   *  rejects (401) or, per `onUnreachable`, one whose node is down. Returns the live session, or null
   *  if none survived (→ login screen). Call this on boot in place of `session.restore()`. */
  restore(): Promise<Session | null>;
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
    async restore() {
      await session.restore();
      if (!session.current()) return null; // nothing stored — straight to login
      const liveness = await probeSession(config);
      // Dead token → always drop (it will 401 every read anyway). Unreachable node → drop unless the
      // caller opted to keep an offline session. `session.logout(ws)` drops just the active
      // workspace's session and notifies subscribers, so `useSession` re-renders into login.
      if (liveness === "dead" || (liveness === "unreachable" && opts.onUnreachable !== "keep")) {
        const ws = session.current()?.workspace;
        if (ws) session.logout(ws);
        return null;
      }
      return session.current();
    },
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
