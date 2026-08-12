import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Device, NotFound } from "./types";

/** `ros.device.get {ros_uuid, network_uuid, device_uuid}` — one device, for breadcrumb labels. */
export function useDevice(bridge: PageBridge) {
  return useAsyncAction((rosUuid: string, networkUuid: string, deviceUuid: string) =>
    bridge.call<Device | NotFound>("ros.device.get", {
      ros_uuid: rosUuid,
      network_uuid: networkUuid,
      device_uuid: deviceUuid,
    }),
  );
}
