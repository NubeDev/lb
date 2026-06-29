// The chain DAG canvas (rules-workbench scope, Phase 2). React Flow renders the open chain: nodes =
// steps, edges = `needs`. An author connects nodes to wire dependencies and adds steps; Save calls
// `chains.save` — a cyclic/invalid edge renders the host's `400` message INLINE (no crash). Run calls
// `chains.run` then the canvas polls `chains.runs.get` and paints each node as it settles
// (pending → running → ok | err | skipped, the Halt-pruned subtree greyed), with a terminal-status
// banner. The node/edge model maps 1:1 to `chain.steps[].needs` (a faithful save/load).

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  Background,
  Controls,
  ReactFlow,
  type Connection,
  type Edge,
  type EdgeChange,
  type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

import { Button } from "@/components/ui/button";
import { runChain, type Chain } from "@/lib/chains";
import {
  chainToEdges,
  chainToNodes,
  nodesToSteps,
  snapshotColours,
  type StepFlowNode,
} from "./chainGraph";
import { StepNode } from "./StepNode";
import { useChainRun } from "./useChainRun";

const nodeTypes = { step: StepNode };

export interface ChainCanvasProps {
  chain: Chain;
  /** Persist the edited DAG. Returns the host's validation outcome so the canvas can show it inline. */
  onSave: (chain: Chain) => Promise<{ ok: boolean; error?: string }>;
}

export function ChainCanvas({ chain, onSave }: ChainCanvasProps) {
  const [nodes, setNodes] = useState<StepFlowNode[]>(() => chainToNodes(chain));
  const [edges, setEdges] = useState<Edge[]>(() => chainToEdges(chain));
  const [saveError, setSaveError] = useState<string | null>(null);
  const [runId, setRunId] = useState<string | null>(null);
  const [runError, setRunError] = useState<string | null>(null);

  // Re-seed the canvas when a different chain opens (id changes) — a faithful load.
  useEffect(() => {
    setNodes(chainToNodes(chain));
    setEdges(chainToEdges(chain));
    setSaveError(null);
    setRunId(null);
    setRunError(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chain.id]);

  const { snapshot } = useChainRun(runId ? chain.id : null, runId);

  // Paint nodes from the live run snapshot as steps settle.
  const colours = useMemo(() => (snapshot ? snapshotColours(snapshot) : {}), [snapshot]);
  const paintedNodes = useMemo(
    () => nodes.map((n) => ({ ...n, data: { ...n.data, colour: colours[n.id] ?? "pending" } })),
    [nodes, colours],
  );

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => setNodes((ns) => applyNodeChanges(changes, ns) as StepFlowNode[]),
    [],
  );
  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => setEdges((es) => applyEdgeChanges(changes, es)),
    [],
  );
  const onConnect = useCallback(
    (c: Connection) => setEdges((es) => addEdge(c, es)),
    [],
  );

  const handleSave = useCallback(async () => {
    const steps = nodesToSteps(nodes, edges, chain);
    const next: Chain = { ...chain, steps };
    const res = await onSave(next);
    setSaveError(res.ok ? null : res.error ?? "save failed");
  }, [nodes, edges, chain, onSave]);

  const handleRun = useCallback(async () => {
    setRunError(null);
    try {
      const { run_id } = await runChain(chain.id);
      setRunId(run_id);
    } catch (e) {
      setRunError(e instanceof Error ? e.message : String(e));
    }
  }, [chain.id]);

  return (
    <div aria-label="chain canvas" style={{ flex: 1, display: "flex", flexDirection: "column" }}>
      <div style={{ display: "flex", gap: 8, padding: 8, alignItems: "center" }}>
        <strong>{chain.name || chain.id}</strong>
        <Button aria-label="save chain" onClick={handleSave} variant="outline" size="sm">
          Save
        </Button>
        <Button aria-label="run chain" onClick={handleRun} size="sm">
          Run
        </Button>
        {saveError ? (
          <span aria-label="chain save error" style={{ color: "#dc2626", fontSize: 12 }}>
            {saveError}
          </span>
        ) : null}
        {runError ? (
          <span aria-label="chain run error" style={{ color: "#dc2626", fontSize: 12 }}>
            {runError}
          </span>
        ) : null}
        {snapshot ? (
          <span aria-label="run status" data-status={snapshot.status} style={{ fontSize: 12 }}>
            {snapshot.status}
          </span>
        ) : null}
      </div>
      <div style={{ flex: 1, minHeight: 300 }}>
        <ReactFlow
          nodes={paintedNodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          fitView
        >
          <Background />
          <Controls />
        </ReactFlow>
      </div>
    </div>
  );
}
