// The single IPC seam to a node. TWO real transports, chosen by environment — the feature code
// above NEVER branches on which:
//   1. Tauri shell        → a Rust command (the node runs in-process behind it).
//   2. Browser + gateway  → real HTTP to the node's SSE/HTTP gateway (S3+). Always a REAL node.
//
// There is **no fake transport** any more (the `*.fake.ts` parallel backend is deleted — CLAUDE §9,
// testing §0). A fake let work *look* shipped on an unbuilt path and an AI couldn't tell fake from
// real; it is gone. The browser always talks to a real gateway (`VITE_GATEWAY_URL`, defaulting to the
// local dev node); tests talk to a real spawned node via the real-gateway harness
// (`src/test/real-gateway.ts` + `vitest.gateway.config.ts`). Outside both — no Tauri, no gateway —
// `invoke` THROWS rather than silently returning fabricated data.

import { httpInvoke, gatewayUrl } from "./http";

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

/** Invoke a node command by name. Mirrors the Rust command names (`channel_post`, …). Always hits a
 *  REAL node — the Tauri host or the HTTP gateway. Throws if neither is reachable (no fake fallback). */
export function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri()) return tauriInvoke<T>(cmd, args);
  if (gatewayUrl() !== "") return httpInvoke<T>(cmd, args);
  return Promise.reject(
    new Error(
      `no node transport for "${cmd}": not in Tauri and no gateway configured. ` +
        `Set VITE_GATEWAY_URL (the browser build) or run inside the Tauri shell.`,
    ),
  );
}
