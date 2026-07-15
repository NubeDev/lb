// The one client seam to the node — `invoke(cmd, args)`, mapping verb→route **1:1 with
// `ui/src/lib/ipc/http.ts`** so the web and app shells cannot drift (app-sdk scope). This slice
// carries only the verbs the app shell needs (shell slice); later slices extend the map, never fork
// it. Feature code never sees the transport; the app adds NO new gateway verbs or caps.

import type { GatewayConfig } from "./config";
import { getJson, postJson } from "./request";

const enc = encodeURIComponent;

/** The `undo`/`redo` body: an optional `surface` stack key, omitted entirely when absent. */
function surfaceBody(args?: Record<string, unknown>): Record<string, unknown> {
  const surface = args?.surface;
  return typeof surface === "string" ? { surface } : {};
}

/** A bound `invoke` for one gateway. Command names mirror the web shell's (`channel_post`, …). */
export type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;

/** Bind `invoke` to a gateway config. Unknown commands throw — never a silent fallback. */
export function createInvoke(config: GatewayConfig): Invoke {
  return async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    switch (cmd) {
      case "login": {
        const { user, workspace } = args as { user: string; workspace: string };
        return postJson<T>(config, "/login", { user, workspace }, /* auth */ false);
      }
      case "workspace_list":
        return getJson<T>(config, "/workspaces");
      case "workspace_create": {
        const { ws, name } = args as { ws: string; name: string };
        return postJson<T>(config, "/workspaces", { ws, name });
      }
      case "channel_list":
        return getJson<T>(config, "/channels");
      case "channel_create": {
        const { channel } = args as { channel: string };
        return postJson<T>(config, "/channels", { channel });
      }
      case "channel_post": {
        const { channel, item } = args as { channel: string; item: unknown };
        return postJson<T>(config, `/channels/${enc(channel)}/messages`, item);
      }
      case "channel_history": {
        const { channel } = args as { channel: string };
        return getJson<T>(config, `/channels/${enc(channel)}/messages`);
      }
      case "ext_list":
        return getJson<T>(config, "/extensions");
      case "mcp_call": {
        const { tool, args: toolArgs } = args as { tool: string; args?: unknown };
        return postJson<T>(config, "/mcp/call", { tool, args: toolArgs ?? {} });
      }
      // Undo journal (undo-exposure scope). `surface` is the optional finer stack key; the
      // workspace + actor come from the token, so a session only ever reaches its own stack.
      // An undo/redo that cannot apply returns `{ok:false, reason}` — data the shell renders,
      // not an error; only a capability failure throws (`InvokeError.isDenied`).
      case "undo":
        return postJson<T>(config, "/undo", surfaceBody(args));
      case "redo":
        return postJson<T>(config, "/redo", surfaceBody(args));
      case "undo_history":
        return getJson<T>(config, "/undo/history");
      case "undo_compensations": {
        const { seq } = args as { seq: number };
        return getJson<T>(config, `/undo/history/${enc(String(seq))}/compensations`);
      }
      default:
        throw new Error(`unknown command: ${cmd}`);
    }
  };
}
