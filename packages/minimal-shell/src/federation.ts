// Federation loader — the host half of the @nube/ext-ui-sdk's defineRemote contract.
// Loads a remote entry (the extension's UI bundle) and calls its mount() function.

import type { RemoteMount } from "@nube/ext-ui-sdk";
import { gatewayUrl } from "./ipc";

export interface ExtPage {
  ext: string;
  entry: string;
  label?: string;
}

export interface PageBridge {
  call: (tool: string, args?: unknown) => Promise<unknown>;
}

export interface PageCtx {
  workspace: string;
}

let moduleCache: Map<string, Promise<{ mount: RemoteMount }>> = new Map();

export function loadRemoteMount(ext: string, entry: string): Promise<{ mount: RemoteMount }> {
  const url = `${gatewayUrl()}/extensions/${encodeURIComponent(ext)}/ui/${entry}`;
  if (moduleCache.has(url)) return moduleCache.get(url)!;
  const p = (async () => {
    const mod = await import(/* @vite-ignore */ url);
    const mount = mod.mount ?? mod.default?.mount ?? mod.default;
    if (typeof mount !== "function") throw new Error(`remote ${ext}: no mount() export`);
    return { mount };
  })();
  moduleCache.set(url, p);
  return p;
}

export function makeBridge(allowedTools: string[]): PageBridge {
  return {
    call: async (tool: string, args?: unknown) => {
      if (allowedTools.length > 0 && !allowedTools.includes(tool)) {
        throw new Error(`bridge: tool not in scope: ${tool}`);
      }
      const { mcpCall } = await import("./ipc");
      return mcpCall(tool, args ?? {});
    },
  };
}
