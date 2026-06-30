// The typed-node DAG canvas (flows-canvas scope, Wave 3). React Flow renders the open flow: nodes =
// typed graph nodes, edges = `needs`. An author drags node types from the palette onto the canvas,
// wires them, and configures each via the schema-rendered side panel. Save calls `flows.save` — a
// cyclic/invalid DAG or schema-invalid node config renders the host's `400` message INLINE (no
// crash). Run calls `flows.run` then the canvas polls `flows.runs.get` (bounded) and paints each node
// as it settles, with suspend/resume/cancel + the executed-node-lock + `flows.patch_run` for
// unexecuted nodes (Decision 1/12). Import/export round-trips the flow JSON; undo restores a prior
// graph (node + edges) atomically by re-saving the previous version.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
import {
  cancelFlow,
  deleteFlow,
  patchFlowRun,
  resumeFlow,
  runFlow,
  suspendFlow,
  type Flow,
  type FlowNode,
  type NodeDescriptor,
} from "@/lib/flows";
import {
  executedNodeIds,
  flowToEdges,
  flowToNodes,
  nodesToFlowNodes,
  snapshotColours,
  type FlowCanvasNode,
} from "./flowGraph";
import { FlowNodeView } from "./FlowNodeView";
import { NodeConfigPanel } from "./NodeConfigPanel";
import { Palette } from "./Palette";
import { useFlowRun } from "./useFlowRun";

const nodeTypes = { flow: FlowNodeView };

export interface FlowCanvasProps {
  flow: Flow;
  palette: NodeDescriptor[];
  /** Persist the edited flow via the parent (which refreshes the roster). Surfaces the host outcome. */
  onSave: (flow: Flow) => Promise<{ ok: boolean; version?: number; error?: string }>;
  /** Notify the parent the flow was deleted (closes the canvas). */
  onDeleted: () => void;
}

/** A transient undo stack entry — the graph BEFORE a save. Undo re-saves it, restoring a deleted
 *  node + its edges atomically (a flow is one record, so the whole-graph revert is one write). */
interface UndoEntry {
  nodes: FlowNode[];
  label: string;
}

export function FlowCanvas({ flow, palette, onSave, onDeleted }: FlowCanvasProps) {
  const [nodes, setNodes] = useState<FlowCanvasNode[]>(() => flowToNodes(flow));
  const [edges, setEdges] = useState<Edge[]>(() => flowToEdges(flow));
  // Per-node config edit buffer (the flow record's configs, held in component state per the scope's
  // "transient unsaved buffer" allowance — no client-durable graph).
  const [configs, setConfigs] = useState<Record<string, Record<string, unknown>>>(() =>
    indexConfigs(flow),
  );
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [panelError, setPanelError] = useState<string | null>(null);
  const [undoStack, setUndoStack] = useState<UndoEntry[]>([]);
  const importedFile = useRef<HTMLInputElement>(null);

  const { snapshot, error: runError, watch, reattach } = useFlowRun();

  // Re-seed the canvas when a different flow opens — a faithful load + reattach to an active run.
  useEffect(() => {
    setNodes(flowToNodes(flow));
    setEdges(flowToEdges(flow));
    setConfigs(indexConfigs(flow));
    setSelectedId(null);
    setSaveError(null);
    setPanelError(null);
    setUndoStack([]);
    void reattach(flow.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [flow.id]);

  // Paint nodes from the live run snapshot + apply the executed-node-lock.
  const colours = useMemo(() => (snapshot ? snapshotColours(snapshot) : {}), [snapshot]);
  const locked = useMemo(
    () => (snapshot ? executedNodeIds(snapshot) : new Set<string>()),
    [snapshot],
  );
  const runActive = !!snapshot && !isTerminalSnapshot(snapshot);

  const paintedNodes = useMemo(
    () =>
      nodes.map((n) => ({
        ...n,
        data: {
          ...n.data,
          colour: colours[n.id] ?? "pending",
          locked: locked.has(n.id),
        },
      })),
    [nodes, colours, locked],
  );

  const onNodesChange = useCallback(
    (changes: NodeChange[]) =>
      setNodes((ns) => applyNodeChanges(changes, ns) as FlowCanvasNode[]),
    [],
  );
  const onEdgesChange = useCallback(
    (changes: EdgeChange[]) => setEdges((es) => applyEdgeChanges(changes, es)),
    [],
  );
  const onConnect = useCallback((c: Connection) => setEdges((es) => addEdge(c, es)), []);

  /** Add a node instance from the palette (drag-drop or click). Records an undo entry BEFORE the
   *  change so a later undo restores the prior graph (node + its future edges) atomically. */
  const addNode = useCallback(
    (desc: NodeDescriptor) => {
      pushUndo("add node");
      const id = `${desc.type.split(".").pop() ?? desc.type}-${nodes.length + 1}`;
      setNodes((ns) => [
        ...ns,
        { id, type: "flow", position: { x: 360, y: 120 + ns.length * 20 }, data: { type: desc.type, colour: "pending", locked: false, gated: false } },
      ]);
      setConfigs((c) => ({ ...c, [id]: {} }));
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [nodes.length],
  );

  const onDeleteNode = useCallback(
    (id: string) => {
      pushUndo("delete node");
      setNodes((ns) => ns.filter((n) => n.id !== id));
      setEdges((es) => es.filter((e) => e.source !== id && e.target !== id));
      setSelectedId((cur) => (cur === id ? null : cur));
      setConfigs((c) => {
        const next = { ...c };
        delete next[id];
        return next;
      });
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  function pushUndo(label: string) {
    setUndoStack((s) => [
      ...s,
      { nodes: nodesToFlowNodes(nodes, edges, flow), label },
    ]);
  }

  const buildFlow = useCallback(
    (overrideNodes?: FlowNode[]): Flow => ({
      ...flow,
      nodes: overrideNodes ?? nodesToFlowNodes(nodes, edges, flow).map((n) => ({ ...n, config: configs[n.id] ?? n.config })),
    }),
    [flow, nodes, edges, configs],
  );

  /** Save the current graph (a new version — Decision 1). Pushes the prior graph onto the undo stack
   *  so a later undo reverts the whole edit atomically. */
  const handleSave = useCallback(async (): Promise<{ ok: boolean; error?: string }> => {
    const prior = { nodes: nodesToFlowNodes(nodes, edges, flow), label: "before save" };
    const next = buildFlow();
    const res = await onSave(next);
    if (res.ok) {
      setUndoStack((s) => [...s, prior]);
      setSaveError(null);
      setPanelError(null);
    } else {
      setSaveError(res.error ?? "save failed");
      setPanelError(res.error ?? "save failed");
    }
    return res;
  }, [buildFlow, flow, nodes, edges, onSave]);

  /** Undo: re-save the prior graph (the top of the undo stack). Atomic — a flow is one record, so a
   *  deleted node + its edges return together. */
  const handleUndo = useCallback(async () => {
    const entry = undoStack[undoStack.length - 1];
    if (!entry) return;
    const next: Flow = { ...flow, nodes: entry.nodes.map((n) => ({ ...n, config: configs[n.id] ?? n.config })) };
    const res = await onSave(next);
    if (res.ok) {
      setUndoStack((s) => s.slice(0, -1));
      setSaveError(null);
    } else {
      setSaveError(res.error ?? "undo failed");
    }
  }, [undoStack, flow, configs, onSave]);

  const handleRun = useCallback(async () => {
    setSaveError(null);
    try {
      const { run_id } = await runFlow(flow.id);
      watch(run_id);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    }
  }, [flow.id, watch]);

  const handleLifecycle = useCallback(
    async (op: "suspend" | "resume" | "cancel") => {
      if (!snapshot) return;
      try {
        if (op === "suspend") await suspendFlow(snapshot.runId);
        if (op === "resume") await resumeFlow(snapshot.runId);
        if (op === "cancel") await cancelFlow(snapshot.runId);
      } catch (e) {
        setSaveError(e instanceof Error ? e.message : String(e));
      }
    },
    [snapshot],
  );

  /** Config-only patch to the selected unexecuted node of the live run (Decision 1/12). The host
   *  validates against the run's PINNED schema; a mismatch surfaces as a 400 inline error. */
  const handlePatch = useCallback(async (): Promise<{ ok: boolean; error?: string }> => {
    if (!snapshot || !selectedId) return { ok: false, error: "no selection" };
    try {
      await patchFlowRun(snapshot.runId, selectedId, configs[selectedId] ?? {});
      setPanelError(null);
      return { ok: true };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setPanelError(msg);
      return { ok: false, error: msg };
    }
  }, [snapshot, selectedId, configs]);

  // Import/export the flow JSON (graph + node configs + version). Import re-validates via save.
  const handleExport = useCallback(() => {
    const blob = new Blob([JSON.stringify(buildFlow(), null, 2)], {
      type: "application/json",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${flow.id}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }, [buildFlow, flow.id]);

  const handleImport = useCallback(
    async (file: File) => {
      try {
        const text = await file.text();
        const imported = JSON.parse(text) as Flow;
        const next: Flow = { ...imported, id: flow.id, workspace: flow.workspace };
        // Re-validate through the real save path (schema + DAG). Surfaces a 400 inline on mismatch.
        const res = await onSave(next);
        if (!res.ok) setSaveError(res.error ?? "import failed");
      } catch (e) {
        setSaveError(e instanceof Error ? e.message : String(e));
      }
    },
    [flow.id, flow.workspace, onSave],
  );

  const handleDelete = useCallback(async () => {
    try {
      await deleteFlow(flow.id);
      onDeleted();
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    }
  }, [flow.id, onDeleted]);

  // A node whose underlying tool the caller lacks is shown-but-marked gated. The caller's reachable
  // tools aren't on the palette response (the descriptor declares no caps); we mark a node gated when
  // its descriptor is absent from the merged registry (the type went stale — an uninstalled ext).
  const descriptorByType = useMemo(
    () => new Map(palette.map((d) => [d.type, d])),
    [palette],
  );
  const selectedNode: FlowNode | null = useMemo(() => {
    if (!selectedId) return null;
    return buildFlow().nodes.find((n) => n.id === selectedId) ?? null;
  }, [selectedId, buildFlow]);
  const selectedDescriptor = selectedNode ? descriptorByType.get(selectedNode.type) ?? null : null;

  return (
    <div aria-label="flow canvas" className="flex flex-1 flex-col">
      <div className="flex items-center gap-2 p-2">
        <strong className="text-sm text-fg">{flow.name || flow.id}</strong>
        <span className="text-xs text-muted">v{flow.version}</span>
        <Button aria-label="save flow" onClick={handleSave} variant="outline" size="sm">
          Save
        </Button>
        <Button aria-label="run flow" onClick={handleRun} size="sm">
          Run
        </Button>
        {runActive ? (
          <>
            <Button aria-label="suspend run" onClick={() => handleLifecycle("suspend")} variant="outline" size="sm">
              Suspend
            </Button>
            <Button aria-label="resume run" onClick={() => handleLifecycle("resume")} variant="outline" size="sm">
              Resume
            </Button>
            <Button aria-label="cancel run" onClick={() => handleLifecycle("cancel")} variant="outline" size="sm">
              Cancel
            </Button>
          </>
        ) : null}
        <Button aria-label="undo" onClick={handleUndo} variant="ghost" size="sm" disabled={undoStack.length === 0}>
          Undo
        </Button>
        <Button aria-label="export flow" onClick={handleExport} variant="ghost" size="sm">
          Export
        </Button>
        <Button
          aria-label="import flow"
          onClick={() => importedFile.current?.click()}
          variant="ghost"
          size="sm"
        >
          Import
        </Button>
        {/* eslint-disable-next-line no-restricted-syntax -- a hidden native file picker; no shadcn equivalent */}
        <input
          ref={importedFile}
          type="file"
          accept="application/json"
          className="hidden"
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) void handleImport(f);
            e.target.value = "";
          }}
        />
        <Button aria-label="delete flow" onClick={handleDelete} variant="ghost" size="sm">
          Delete
        </Button>
        {saveError ? (
          <span aria-label="flow error" className="text-xs text-denied">
            {saveError}
          </span>
        ) : null}
        {runError ? (
          <span aria-label="run error" className="text-xs text-denied">
            {runError}
          </span>
        ) : null}
        {snapshot ? (
          <span aria-label="run status" data-status={snapshot.status} className="text-xs text-fg">
            {snapshot.status}
          </span>
        ) : null}
      </div>
      {snapshot ? (
        <div aria-label="v-pinned banner" className="bg-accent/10 px-3 py-1 text-xs text-fg">
          This run is on v{snapshot.flowVersion}. Structural edits become a new version for the next
          run; executed nodes are read-only.
        </div>
      ) : null}
      <div className="flex flex-1 min-h-0">
        <Palette nodes={palette} onAdd={addNode} />
        <div
          className="flex-1 min-w-0"
          onDragOver={(e) => e.preventDefault()}
          onDrop={(e) => {
            const type = e.dataTransfer.getData("application/x-flow-node");
            const desc = palette.find((d) => d.type === type);
            if (desc) addNode(desc);
          }}
        >
          <ReactFlow
            nodes={paintedNodes}
            edges={edges}
            nodeTypes={nodeTypes}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onNodeClick={(_, n) => setSelectedId(n.id)}
            onNodeDoubleClick={(_, n) => onDeleteNode(n.id)}
            fitView
          >
            <Background />
            <Controls />
          </ReactFlow>
        </div>
        <NodeConfigPanel
          node={selectedNode}
          descriptor={selectedDescriptor}
          locked={selectedId ? locked.has(selectedId) : false}
          runActive={runActive}
          config={selectedId ? configs[selectedId] ?? {} : {}}
          onConfigChange={(next) =>
            selectedId && setConfigs((c) => ({ ...c, [selectedId]: next }))
          }
          onSave={handleSave}
          onPatch={handlePatch}
          onClose={() => setSelectedId(null)}
          error={panelError}
        />
      </div>
    </div>
  );
}

/** Index a flow's node configs into the per-node edit buffer. */
function indexConfigs(flow: Flow): Record<string, Record<string, unknown>> {
  const out: Record<string, Record<string, unknown>> = {};
  for (const n of flow.nodes) {
    out[n.id] = (n.config as Record<string, unknown> | null | undefined) ?? {};
  }
  return out;
}

function isTerminalSnapshot(s: { status: string }): boolean {
  return s.status === "success" || s.status === "partialFailure" || s.status === "failed";
}
