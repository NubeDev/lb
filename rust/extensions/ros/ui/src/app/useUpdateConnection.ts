import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Connection } from "./types";

export interface UpdateConnectionInput {
  uuid: string;
  name?: string;
  base_url?: string;
  enable?: boolean;
  poll_rate?: number;
  /** Re-stashed and never echoed back if present. */
  token?: string;
}

/** `ros.update` — rename / re-point / toggle enable / re-token. Admin-only. */
export function useUpdateConnection(bridge: PageBridge) {
  return useAsyncAction((input: UpdateConnectionInput) =>
    bridge.call<Connection>("ros.update", input as unknown as Record<string, unknown>),
  );
}
