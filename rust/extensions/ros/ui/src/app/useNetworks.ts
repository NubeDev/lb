import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Page, Network } from "./types";

/** `network.list {ros_uuid}` — keyset-paged networks under a connection. */
export function useNetworks(bridge: PageBridge) {
  return useAsyncAction((rosUuid: string, cursor?: string) =>
    bridge.call<Page<Network>>("ros.network.list", { ros_uuid: rosUuid, ...(cursor ? { cursor } : {}) }),
  );
}
