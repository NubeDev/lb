// The single IPC seam to the local node. In the Tauri shell this calls a Rust command (the
// node runs in-process behind it). Outside Tauri (a plain browser during S2, or a test) there
// is no node yet — SSE/HTTP is S3 — so it routes to an in-memory fake with the same contract.
//
// One seam, swapped by environment: the feature code above never branches on "am I in Tauri".
// That is what lets the same ChannelView power the desktop shell and a test unchanged.

import { fakeInvoke } from "./fake";

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

/** True when running inside the Tauri shell (the global is injected by Tauri v2). */
function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

let real: Invoke | null = null;

/** Lazily load the Tauri `invoke` only when actually in the shell (keeps it out of the web
 *  bundle's hot path and out of tests). */
async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!real) {
    const mod = await import("@tauri-apps/api/core");
    real = mod.invoke as Invoke;
  }
  return real<T>(cmd, args);
}

/** Invoke a node command by name. Mirrors the Rust command names (`channel_post`, …). */
export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri() ? tauriInvoke<T>(cmd, args) : fakeInvoke<T>(cmd, args);
}
