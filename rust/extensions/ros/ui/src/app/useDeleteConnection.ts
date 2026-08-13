import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";

/** `ros.delete` — remove a connection (its secret token too). Admin-only. */
export function useDeleteConnection(bridge: PageBridge) {
  return useAsyncAction((uuid: string) =>
    bridge.call<{ ok: boolean; uuid: string }>("ros.delete", { uuid }),
  );
}
