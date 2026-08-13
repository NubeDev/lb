import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Page, Device } from "./types";

/** `ros.device.list {ros_uuid, network_uuid}` — keyset-paged devices under a network. */
export function useDevices(bridge: PageBridge) {
  return useAsyncAction((rosUuid: string, networkUuid: string, cursor?: string) =>
    bridge.call<Page<Device>>("ros.device.list", {
      ros_uuid: rosUuid,
      network_uuid: networkUuid,
      ...(cursor ? { cursor } : {}),
    }),
  );
}
