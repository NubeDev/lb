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
