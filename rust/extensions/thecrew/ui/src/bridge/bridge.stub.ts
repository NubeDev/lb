// Test doubles for the host bridge (the ONLY external seam the extension has). Copied from
// proof-panel's `test/bridge.stub.ts` — the same seam, a different consumer. These stub the
// FRONTEND'S view of a granted tool set; the REAL host re-check is exercised in the gateway tests.
// A tool absent from the table throws `out_of_scope` — exactly what the shell's `makeBridge` scope
// filter does (defense in depth), so a deny test omits the verb from the table.

import { vi } from "vitest";
import type { Bridge, WidgetBridge } from "./contract";

export type Resolver = (args?: Record<string, unknown>) => unknown;

/** A call-only page bridge backed by a resolver table. Unknown tool → `out_of_scope` (the scope
 *  filter's contract). Async so callers can `await`. */
export function stubBridge(table: Record<string, Resolver>): Bridge {
  return {
    call: vi.fn(async (tool: string, args?: Record<string, unknown>) => {
      if (!(tool in table)) throw new Error(`out_of_scope: ${tool}`);
      return table[tool](args);
    }),
  } as Bridge;
}

/** A bridge whose every call rejects — the server-side deny surfaced to the page. */
export function rejectingBridge(message = "denied"): Bridge {
  return { call: vi.fn(async () => Promise.reject(new Error(message))) } as Bridge;
}

/** A widget bridge with a live `watch`: returns `{ bridge, emit, unsubscribed }` so a test can push
 *  a live sample and assert the tile tore the stream down on unmount. Mirrors proof-panel. */
export function watchBridge(table: Record<string, Resolver>): {
  bridge: WidgetBridge;
  emit: (series: string, sample: unknown) => void;
  unsubscribed: () => boolean;
} {
  const cbs = new Map<string, (e: unknown) => void>();
  let torn = false;
  const bridge = {
    call: vi.fn(async (tool: string, args?: Record<string, unknown>) => {
      if (!(tool in table)) throw new Error(`out_of_scope: ${tool}`);
      return table[tool](args);
    }),
    watch: vi.fn((_tool: string, args: Record<string, unknown>, onEvent: (e: unknown) => void) => {
      const series = String(args.series);
      cbs.set(series, onEvent);
      return () => {
        torn = true;
        cbs.delete(series);
      };
    }),
  };
  return {
    bridge: bridge as unknown as WidgetBridge,
    emit: (series, sample) => cbs.get(series)?.(sample),
    unsubscribed: () => torn,
  };
}
