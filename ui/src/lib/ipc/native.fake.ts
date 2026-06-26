// The in-memory native-tier stand-in used when NOT in the Tauri shell (plain browser, tests). It
// mirrors the node's `native.*` contract faithfully enough that the UI behaves identically here and
// against the real node (the verb names + shapes match the Rust commands one-to-one).
//
// Faithful to the gates + supervision the user actually sees:
//   - the CAPABILITY gate: each verb needs its `mcp:native.<verb>:call` grant, else "denied"
//     (the same opaque deny the Rust `native_deny_test` proves);
//   - WORKSPACE isolation: state is keyed by `${ws}/${extId}`, so one workspace never sees another's
//     sidecar (the hard wall, mirrored — `native_isolation_test`);
//   - SUPERVISION: install spawns (running=true, restartCount=0); restart bumps restartCount and
//     keeps it running (the supervision proof, surfaced); stop flips lifecycle to "stopped".
//
// One file per concern (FILE-LAYOUT): the native fake lives beside the registry/agent/workflow fakes.

import type { NativeStatus } from "@/lib/native/native.types";

const INSTALL_CAP = "mcp:native.install:call";
const STATUS_CAP = "mcp:native.status:call";
const RESTART_CAP = "mcp:native.restart:call";
const STOP_CAP = "mcp:native.stop:call";

// Workspace-scoped state (key = `${ws}/${extId}`) — the hard wall, mirrored.
const sidecars = new Map<string, NativeStatus>();

const k = (ws: string, x: string) => `${ws}/${x}`;

function capMatches(held: string[], cap: string): boolean {
  return held.some((h) => h === cap || h === "mcp:native.*:call" || h === "mcp:*:call");
}

export function nativeFakeInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> | null {
  switch (cmd) {
    case "native_install": {
      const { ws, extId, caps } = args as { ws: string; extId: string; caps?: string[] };
      if (!capMatches(caps ?? [], INSTALL_CAP)) return Promise.reject(new Error("denied"));
      const status: NativeStatus = {
        extId,
        version: "0.1.0",
        lifecycle: "started",
        restartCount: 0,
        running: true,
      };
      sidecars.set(k(ws, extId), status);
      return Promise.resolve({ extId, version: "0.1.0", restartCount: 0 } as T);
    }
    case "native_status": {
      const { ws, extId, caps } = args as { ws: string; extId: string; caps?: string[] };
      if (!capMatches(caps ?? [], STATUS_CAP)) return Promise.reject(new Error("denied"));
      return Promise.resolve((sidecars.get(k(ws, extId)) ?? null) as T);
    }
    case "native_restart": {
      const { ws, extId, caps } = args as { ws: string; extId: string; caps?: string[] };
      if (!capMatches(caps ?? [], RESTART_CAP)) return Promise.reject(new Error("denied"));
      const cur = sidecars.get(k(ws, extId));
      if (!cur) return Promise.reject(new Error("sidecar not running"));
      const next: NativeStatus = {
        ...cur,
        restartCount: cur.restartCount + 1,
        lifecycle: "started",
        running: true,
      };
      sidecars.set(k(ws, extId), next);
      return Promise.resolve({ extId, version: cur.version, restartCount: next.restartCount } as T);
    }
    case "native_stop": {
      const { ws, extId, caps } = args as { ws: string; extId: string; caps?: string[] };
      if (!capMatches(caps ?? [], STOP_CAP)) return Promise.reject(new Error("denied"));
      const cur = sidecars.get(k(ws, extId));
      if (!cur) return Promise.reject(new Error("sidecar not running"));
      const next: NativeStatus = { ...cur, lifecycle: "stopped", running: false };
      sidecars.set(k(ws, extId), next);
      return Promise.resolve(next as T);
    }
    default:
      return null; // not a native command — let the caller fall through
  }
}

/** Test helper: clear all native fake state. */
export function __resetNativeFake(): void {
  sidecars.clear();
}
