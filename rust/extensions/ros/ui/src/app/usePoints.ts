import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Page, Point } from "./types";

/** `ros.point.list {ros_uuid, device_uuid}` — keyset-paged points under a device. */
export function usePoints(bridge: PageBridge) {
  return useAsyncAction((rosUuid: string, deviceUuid: string, cursor?: string) =>
    bridge.call<Page<Point>>("ros.point.list", {
      ros_uuid: rosUuid,
      device_uuid: deviceUuid,
      ...(cursor ? { cursor } : {}),
    }),
  );
}
