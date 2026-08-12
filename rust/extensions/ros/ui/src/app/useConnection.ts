import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";
import type { Connection, NotFound } from "./types";

/** `ros.get` — one connection by uuid (token never returned). */
export function useConnection(bridge: PageBridge) {
  return useAsyncAction((uuid: string) =>
    bridge.call<Connection | NotFound>("ros.get", { uuid }),
  );
}
