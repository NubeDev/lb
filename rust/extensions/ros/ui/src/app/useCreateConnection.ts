import type { PageBridge } from "@nube/ext-ui-sdk";
import { useAsyncAction } from "./useAsyncAction";

export interface CreateConnectionInput {
  uuid: string;
  name: string;
  base_url: string;
  token: string;
  enable?: boolean;
  poll_rate?: number;
}

/** `ros.create` — register an appliance. Caller-supplied uuid (idempotent re-create). Admin-only. */
export function useCreateConnection(bridge: PageBridge) {
  return useAsyncAction((input: CreateConnectionInput) =>
    bridge.call<{ uuid: string }>("ros.create", input as unknown as Record<string, unknown>),
  );
}
