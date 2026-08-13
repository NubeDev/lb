import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Network, NotFound } from "./types";

/** `network.get {ros_uuid, network_uuid}` — one network, for breadcrumb labels. */
export function useNetwork(bridge: PageBridge) {
  return useAsyncAction((rosUuid: string, networkUuid: string) =>
    bridge.call<Network | NotFound>("ros.network.get", { ros_uuid: rosUuid, network_uuid: networkUuid }),
  );
}
