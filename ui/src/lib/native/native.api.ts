// The native-tier API client — one call per export, mirroring the Rust `native.*` verbs and the node
// command name one-to-one. The UI never calls `invoke` directly; it goes through these named verbs
// (FILE-LAYOUT frontend rules).
//
// `author`/`caps` are the caller's demo principal + grant (the real node derives them from the
// session token; the in-memory fake uses them to resolve the capability gate, so the UI's allow/deny
// paths are exercised exactly as the node would — same seam as the registry/workflow api).

import type { NativeStatus, NativeResult } from "./native.types";
import { invoke } from "@/lib/ipc/invoke";

/** Install (spawn + supervise) a native sidecar. Mirrors `native.install`. */
export function installNative(
  ws: string,
  extId: string,
  opts?: { author?: string; caps?: string[] },
): Promise<NativeResult> {
  return invoke<NativeResult>("native_install", { ws, extId, author: opts?.author, caps: opts?.caps });
}

/** Read the durable status + live running flag for a sidecar. Mirrors `native.status`. */
export function nativeStatus(
  ws: string,
  extId: string,
  opts?: { author?: string; caps?: string[] },
): Promise<NativeStatus | null> {
  return invoke<NativeStatus | null>("native_status", {
    ws,
    extId,
    author: opts?.author,
    caps: opts?.caps,
  });
}

/** Operator-restart a sidecar (cooperative stop → re-spawn). Mirrors `native.restart`. */
export function restartNative(
  ws: string,
  extId: string,
  opts?: { author?: string; caps?: string[] },
): Promise<NativeResult> {
  return invoke<NativeResult>("native_restart", { ws, extId, author: opts?.author, caps: opts?.caps });
}

/** Cooperatively stop a sidecar. Mirrors `native.stop`. */
export function stopNative(
  ws: string,
  extId: string,
  opts?: { author?: string; caps?: string[] },
): Promise<NativeStatus> {
  return invoke<NativeStatus>("native_stop", { ws, extId, author: opts?.author, caps: opts?.caps });
}
