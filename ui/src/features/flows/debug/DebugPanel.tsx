// The flow debug panel (debug-node-scope) — Node-RED's debug sidebar as the right dock's Debug tab.
// Tails the flow's `debug` nodes over the SSE stream (useDebugStream), rendering each message via
// DebugMessageRow with a per-node filter + Clear. v1 is motion-only (browser tail from attach onward
// — no replay); with no gateway/EventSource (Tauri/tests) it says so instead of erroring.
//
// Rebuilt for flow-ui-polish: the original file was committed-by-reference in 9260f1a but swallowed
// by a bare `debug` .gitignore pattern (see docs/debugging/frontend/) and never reached the repo.

import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";

import { useDebugStream } from "./useDebugStream";
import { DebugMessageRow } from "./DebugMessageRow";

export function DebugPanel({ flowId }: { flowId: string }) {
  const { messages, available, clear } = useDebugStream(flowId);
  const [nodeFilter, setNodeFilter] = useState<string>("");

  // The attribution filter options — every node that has published since attach.
  const nodes = useMemo(
    () => Array.from(new Set(messages.map((m) => m.node))).sort(),
    [messages],
  );
  const shown = nodeFilter ? messages.filter((m) => m.node === nodeFilter) : messages;

  if (!available) {
    return (
      <div aria-label="debug stream unavailable" className="p-3 text-xs text-muted">
        The live debug stream needs a gateway connection — none is configured in this session.
      </div>
    );
  }

  return (
    <div aria-label="flow debug panel" className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center gap-2 border-b border-border px-3 py-2">
        <Select
          aria-label="filter debug by node"
          value={nodeFilter}
          onChange={(e) => setNodeFilter(e.target.value)}
          className="h-7 flex-1 text-xs"
        >
          <option value="">All nodes</option>
          {nodes.map((n) => (
            <option key={n} value={n}>
              {n}
            </option>
          ))}
        </Select>
        <Button aria-label="clear debug messages" onClick={clear} variant="ghost" size="sm">
          Clear
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {shown.length === 0 ? (
          <div className="p-3 text-xs text-muted">
            Waiting for messages — wire a <span className="font-mono">debug</span> node to any
            output and run the flow. Messages appear here from the moment this panel opened.
          </div>
        ) : (
          shown.map((m, i) => <DebugMessageRow key={i} msg={m} />)
        )}
      </div>
    </div>
  );
}
