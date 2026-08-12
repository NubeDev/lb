import type { PageBridge, WidgetBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Schedule, NotFound } from "./types";

/** `ros.schedule.get {ros_uuid, schedule_uuid}` — one schedule. Shared by the page drill-down and the
 *  Point Write widget's schedule mode — both bridges expose the same `call` shape. */
export function useSchedule(bridge: PageBridge | WidgetBridge) {
  return useAsyncAction((rosUuid: string, scheduleUuid: string) =>
    bridge.call<Schedule | NotFound>("ros.schedule.get", {
      ros_uuid: rosUuid,
      schedule_uuid: scheduleUuid,
    }),
  );
}
