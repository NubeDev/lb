import type { PageBridge, WidgetBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";

export interface WritePointInput {
  ros_uuid: string;
  point_uuid: string;
  /** Priority slot 1-16 (lower number wins). */
  slot: number;
  /** `null` releases the slot. */
  value: number | null;
}

export interface WritePointResult {
  effect_id: string;
  status: "pending";
}

/** `ros.point.write` — stage a must-deliver setpoint as an outbox effect (never inline). Admin-only. */
export function useWritePoint(bridge: PageBridge | WidgetBridge) {
  return useAsyncAction((input: WritePointInput) =>
    bridge.call<WritePointResult>("ros.point.write", input as unknown as Record<string, unknown>),
  );
}
