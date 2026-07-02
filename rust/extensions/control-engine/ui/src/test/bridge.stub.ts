import { vi } from "vitest";

import type { Bridge } from "@/contract";

// A TEST DOUBLE of the bridge INTERFACE the shell provides — NOT a fake node (rule 9). The real node
// isn't in this remote's process, so tests pass an in-memory bridge whose `call` resolves granted tools
// from a table (an ungranted tool rejects `out_of_scope`, exercising the honest denied path) and whose
// optional `watch` hands back an `emit` to push seeded S6 frames synchronously. The BridgeTransport and
// frames decoder — the only things S7 authored — are what these doubles exercise for real.

type Resolver = (args?: Record<string, unknown>) => unknown;

/** A recording bridge with NO `watch` (the Tauri/vitest posture). `calls` captures every `{tool,args}`. */
export function stubBridge(table: Record<string, Resolver>): Bridge & {
  calls: Array<{ tool: string; args?: Record<string, unknown> }>;
} {
  const calls: Array<{ tool: string; args?: Record<string, unknown> }> = [];
  const bridge = {
    calls,
    call: vi.fn(async <T,>(tool: string, args?: Record<string, unknown>): Promise<T> => {
      calls.push({ tool, args });
      const fn = table[tool];
      if (!fn) throw new Error(`out_of_scope: ${tool}`);
      return fn(args) as T;
    }),
  };
  return bridge as Bridge & typeof bridge;
}

/** A watch-capable bridge double: `call` resolves from `table`; `emit(sample)` pushes a live sample to
 *  the latest `watch` subscriber, and `unsubscribed()` reports whether the transport tore the stream
 *  down. Mirrors proof-panel's `watchBridge` — the shell supplies `watch` over the series SSE. */
export function watchBridge(table: Record<string, Resolver>): {
  bridge: Bridge;
  calls: Array<{ tool: string; args?: Record<string, unknown> }>;
  emit: (sample: unknown) => void;
  unsubscribed: () => boolean;
} {
  const calls: Array<{ tool: string; args?: Record<string, unknown> }> = [];
  let cb: ((e: unknown) => void) | null = null;
  let torn = false;
  const bridge = {
    call: vi.fn(async <T,>(tool: string, args?: Record<string, unknown>): Promise<T> => {
      calls.push({ tool, args });
      const fn = table[tool];
      if (!fn) throw new Error(`out_of_scope: ${tool}`);
      return fn(args) as T;
    }),
    watch: vi.fn((_tool: string, _args: Record<string, unknown>, onEvent: (e: unknown) => void) => {
      cb = onEvent;
      return () => {
        torn = true;
        cb = null;
      };
    }),
  } as unknown as Bridge;
  return {
    bridge,
    calls,
    emit: (sample) => cb?.(sample),
    unsubscribed: () => torn,
  };
}
