// The single IPC seam to a node. Three transports, chosen by environment — the feature code
// above NEVER branches on which:
//   1. Tauri shell        → a Rust command (the node runs in-process behind it).
//   2. Browser + gateway  → real HTTP to the node's SSE/HTTP gateway (S3). Selected when
//                           `VITE_GATEWAY_URL` is set (the browser build).
//   3. Otherwise (tests)  → the in-memory faithful fake, same contract.
//
// S3 swaps the *browser* path from the fake to real HTTP — exactly the one-file change the
// frontend scope promised. Tests still get the fake (no gateway URL), so the Vitest suite is
// unchanged; the Tauri path is unchanged. Live updates ride a separate SSE stream
// (`channel.stream.ts`) feeding `useChannel`'s existing `setItems` sink.

import { fakeInvoke } from "./fake";
import { httpInvoke, gatewayUrl } from "./http";

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

/** True when running inside the Tauri shell (the global is injected by Tauri v2). */
function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** True when a real node gateway is configured (the browser build). */
function hasGateway(): boolean {
  return gatewayUrl() !== "" || import.meta.env.VITE_GATEWAY_URL !== undefined;
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
  if (inTauri()) return tauriInvoke<T>(cmd, args);
  if (hasGateway()) return httpInvoke<T>(cmd, args);
  return fakeInvoke<T>(cmd, args);
}
