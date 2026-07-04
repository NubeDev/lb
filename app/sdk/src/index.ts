// @nube/app-sdk — barrel (re-exports only, per FILE-LAYOUT).

// Mount contracts (app-extensions slice builds on these).
export type { MountCtx, Bridge } from "./contract/mount";
export type {
  WidgetCtx,
  WidgetFrame,
  WidgetBridge,
  WidgetHandleProps,
} from "./contract/widget";
export type { AppRemote } from "./contract/remote";

// The gateway client (shell slice).
export { createGatewayClient } from "./client/create";
export type { GatewayClient, GatewayClientOptions } from "./client/create";
export type { GatewayConfig } from "./client/config";
export { createInvoke } from "./client/invoke";
export type { Invoke } from "./client/invoke";
export { InvokeError } from "./client/errors";

// Live streams.
export { openSse } from "./sse/stream";
export type { SseStream, SseHandlers } from "./sse/stream";
export { openChannelStream } from "./sse/channel.stream";
export type { ChannelStreamHandlers } from "./sse/channel.stream";

// Session.
export { createSessionStore } from "./session/session.store";
export type { SessionStore } from "./session/session.store";
export { memorySessionStorage } from "./session/session.storage";
export type { SessionStorage } from "./session/session.storage";
export { probeSession } from "./session/validate";
export type { SessionLiveness } from "./session/validate";
export type { Session, StoredSessions } from "./session/session.types";

// Domain types + nav derivation.
export type { Item, ChannelRecord } from "./channel/channel.types";
export type { ExtRow, ExtUi } from "./ext/ext.types";
export { extNavEntries, holdsToolCap } from "./ext/nav";
export type { ExtNavEntry } from "./ext/nav";
