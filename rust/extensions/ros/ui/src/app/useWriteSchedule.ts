import type { PageBridge, WidgetBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";

export interface WriteScheduleInput {
  ros_uuid: string;
  schedule_uuid: string;
  schedule: Record<string, unknown>;
}

export interface WriteScheduleResult {
  effect_id: string;
  status: "pending";
}

/** `ros.schedule.write` — stage a must-deliver schedule payload as an outbox effect (never inline).
 *  Admin-only, same contract as `useWritePoint`. */
export function useWriteSchedule(bridge: PageBridge | WidgetBridge) {
  return useAsyncAction((input: WriteScheduleInput) =>
    bridge.call<WriteScheduleResult>("ros.schedule.write", input as unknown as Record<string, unknown>),
  );
}
