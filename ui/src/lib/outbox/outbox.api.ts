// The outbox api client — one call, mirroring `lb_host::outbox_status` and the gateway `GET /outbox`
// (collaboration scope, slice 4). Read-only by design: the outbox is must-deliver infrastructure,
// never a CRUD surface, so there is no enqueue/mark/delete verb here.

import type { OutboxStatus } from "./outbox.types";
import { invoke } from "@/lib/ipc/invoke";

/** Read the delivery status snapshot for the session workspace. Mirrors `outbox_status`. */
export function outboxStatus(): Promise<OutboxStatus> {
  return invoke<OutboxStatus>("outbox_status", {});
}
