import type { PageBridge, WidgetBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Point, NotFound } from "./types";

/** `ros.point.get {ros_uuid, point_uuid}` — one point's live present_value. Shared by the page
 *  drill-down and the Point Value widget — both bridges expose the same `call` shape. */
export function usePoint(bridge: PageBridge | WidgetBridge) {
  return useAsyncAction((rosUuid: string, pointUuid: string) =>
    bridge.call<Point | NotFound>("ros.point.get", { ros_uuid: rosUuid, point_uuid: pointUuid }),
  );
}
