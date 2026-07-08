export type Bridge = {
  call: <T = unknown>(tool: string, args?: Record<string, unknown>) => Promise<T>;
};

let currentBridge: Bridge | null = null;

export function setBridge(b: Bridge) {
  currentBridge = b;
}

export const bridge: Bridge = {
  call: (tool, args) => {
    if (!currentBridge) throw new Error("bridge not set");
    return currentBridge.call(tool, args);
  },
};
