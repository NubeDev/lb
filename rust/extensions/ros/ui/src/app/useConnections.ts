import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Page, Connection } from "./types";

/** `ros.list` — keyset-paged connections. */
export function useConnections(bridge: PageBridge) {
  return useAsyncAction((cursor?: string) =>
    bridge.call<Page<Connection>>("ros.list", cursor ? { cursor } : {}),
  );
}
