// The in-memory outbox-status fake (TEST-ONLY) — mirrors the gateway `outbox_status` (read-only).
// Workspace-scoped via the session store. A test seeds effects with `__seedEffect` and advances them
// with `__markDelivered`; the fake groups them by lifecycle exactly as the host `OutboxStatus` does.
// Returns `null` for unowned commands (fake-chain convention).

import type { Effect, OutboxStatus } from "@/lib/outbox/outbox.types";
import { getSession } from "@/lib/session/session.store";

const effects = new Map<string, Map<string, Effect>>(); // ws → (effectId → effect)

function ws(): string {
  return getSession()?.workspace ?? "";
}

/** Test helper: seed a pending effect in `workspace`. */
export function __seedEffect(workspace: string, effect: Effect): void {
  const byId = effects.get(workspace) ?? new Map<string, Effect>();
  byId.set(effect.id, effect);
  effects.set(workspace, byId);
}

/** Test helper: advance an effect to delivered (a real relay outcome). */
export function __markDelivered(workspace: string, id: string): void {
  const e = effects.get(workspace)?.get(id);
  if (e) e.status = "delivered";
}

export function outboxFakeInvoke<T>(cmd: string): T | null {
  if (cmd !== "outbox_status") return null;
  const all = [...(effects.get(ws())?.values() ?? [])].sort((a, b) => a.ts - b.ts);
  const status: OutboxStatus = {
    pending: all.filter((e) => e.status === "pending" || e.status === "failed"),
    delivered: all.filter((e) => e.status === "delivered"),
    dead_lettered: all.filter((e) => e.status === "dead-lettered"),
  };
  return status as T;
}

/** Test helper: clear the fake outbox between tests. */
export function __resetOutboxFake(): void {
  effects.clear();
}
