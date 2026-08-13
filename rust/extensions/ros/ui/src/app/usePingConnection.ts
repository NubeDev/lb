import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";

export interface PingResult {
  ok: boolean;
  health?: string;
  ros?: unknown;
  error?: string;
}

/** `ros.ping` — health-check the appliance (proxies /api/system/ping). */
export function usePingConnection(bridge: PageBridge) {
  return useAsyncAction((uuid: string) => bridge.call<PingResult>("ros.ping", { uuid }));
}
