// The native-tier hook — data + state for a supervised sidecar (FILE-LAYOUT: one hook per file, data
// separated from markup). It drives the capability-checked node verbs: install (spawn + supervise),
// read status (durable lifecycle + restart count + live running flag), operator-restart, and stop —
// each refused by the node if the caller lacks the grant (surfaced, never a silent no-op).

import { useCallback, useEffect, useState } from "react";

import {
  installNative,
  nativeStatus,
  restartNative,
  stopNative,
} from "@/lib/native/native.api";
import type { NativeStatus } from "@/lib/native/native.types";

export interface NativeState {
  /** The current durable status + live running flag, or null if not installed here. */
  status: NativeStatus | null;
  /** Set when the node denied a verb (missing capability) — shown to the user. */
  error: string | null;
  install: () => Promise<void>;
  restart: () => Promise<void>;
  stop: () => Promise<void>;
}

/** Drive a native sidecar in `(ws)` for `extId` as `author` holding `caps` (the demo session
 *  identity + grant until real login lands — see native.api). */
export function useNative(
  ws: string,
  extId: string,
  author: string,
  caps: string[],
): NativeState {
  const [status, setStatus] = useState<NativeStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await nativeStatus(ws, extId, { author, caps }));
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [ws, extId, author, caps]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(
    async (fn: () => Promise<unknown>) => {
      try {
        await fn();
        await refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    },
    [refresh],
  );

  return {
    status,
    error,
    install: () => run(() => installNative(ws, extId, { author, caps })),
    restart: () => run(() => restartNative(ws, extId, { author, caps })),
    stop: () => run(() => stopNative(ws, extId, { author, caps })),
  };
}
