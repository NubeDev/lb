import { vi } from "vitest";

import type { Bridge } from "@/app/contract";

// A TEST DOUBLE of the bridge INTERFACE the shell provides — not a fake node. The real node isn't in
// this remote's process, so tests pass a real in-memory bridge resolving `series.*` to seeded arrays
// (or rejecting, to exercise honest error states). This is the allowed seam per testing-scope §0.

type Resolver = (args?: Record<string, unknown>) => unknown;

/** Build a bridge whose granted tools resolve from `table`; an ungranted tool rejects `out_of_scope`. */
export function stubBridge(table: Record<string, Resolver>): Bridge {
  return {
    call: vi.fn(async <T,>(tool: string, args?: Record<string, unknown>): Promise<T> => {
      const fn = table[tool];
      if (!fn) throw new Error(`out_of_scope: ${tool}`);
      return fn(args) as T;
    }),
  };
}

/** A bridge whose every call rejects — exercises the honest error/denied state. */
export function rejectingBridge(message = "denied"): Bridge {
  return { call: vi.fn(async () => Promise.reject(new Error(message))) };
}

/** The v2 widget bridge interface: `call` + `watch`. The shell supplies `watch` over the series SSE;
 *  here a test double drives the live tile by handing back an `emit` to push samples synchronously. */
export interface WatchBridge {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
  watch: (tool: string, args: Record<string, unknown>, onEvent: (e: unknown) => void) => () => void;
}

/** Build a watch-capable bridge double. `table` resolves `call` (as {@link stubBridge}); the returned
 *  `emit(sample)` pushes a live sample to the latest `watch` subscriber, and `unsubscribed()` reports
 *  whether the tile tore the stream down (stateless eviction). */
export function watchBridge(table: Record<string, Resolver>): {
  bridge: WatchBridge;
  emit: (sample: unknown) => void;
  unsubscribed: () => boolean;
} {
  let cb: ((e: unknown) => void) | null = null;
  let torn = false;
  const bridge: WatchBridge = {
    call: vi.fn(async <T,>(tool: string, args?: Record<string, unknown>): Promise<T> => {
      const fn = table[tool];
      if (!fn) throw new Error(`out_of_scope: ${tool}`);
      return fn(args) as T;
    }),
    watch: vi.fn((_tool, _args, onEvent) => {
      cb = onEvent;
      return () => {
        torn = true;
        cb = null;
      };
    }),
  };
  return {
    bridge,
    emit: (sample) => cb?.(sample),
    unsubscribed: () => torn,
  };
}
