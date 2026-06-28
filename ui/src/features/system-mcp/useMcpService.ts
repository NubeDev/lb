// The MCP service page hook — data + state for the tool-catalog console (tool-catalog scope). Loads
// the runtime summary (`system.overview`, for the MCP card's extension/tool counts) and the full tool
// catalog (`system.tools`) on open, and exposes a `refresh` that re-reads both. READ-ONLY: the page
// lists tools, it never calls them. Poll-on-open (no live feed — the tool set changes only on
// install/reload, which is rare + operator-driven). One hook per file (FILE-LAYOUT). Everything runs
// against the real gateway, admin-gated server-side.

import { useCallback, useEffect, useState } from "react";

import { systemOverview, systemTools } from "@/lib/system/system.api";
import type { ServiceStatus, ToolInfo } from "@/lib/system/system.types";

export interface McpServiceState {
  /** The MCP runtime card (extension + tool counts), from the overview snapshot. Null until loaded. */
  mcpCard: ServiceStatus | null;
  /** Every reachable MCP tool — host-native + extension-contributed — with descriptions. */
  tools: ToolInfo[];
  error: string | null;
  loading: boolean;
  refresh: () => Promise<void>;
}

/** Drive the MCP service page for the session workspace. */
export function useMcpService(): McpServiceState {
  const [mcpCard, setMcpCard] = useState<ServiceStatus | null>(null);
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      // Both reads project from live node state; fetched together so the count card and the list agree.
      const [ov, cat] = await Promise.all([systemOverview(), systemTools()]);
      setMcpCard(ov.services.find((s) => s.id === "mcp") ?? null);
      setTools(cat.tools);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return { mcpCard, tools, error, loading, refresh: load };
}
