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

import {
  Download,
  Play,
  RotateCcw,
  Save,
  Square,
  Trash2,
  Upload,
  Pause,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import {
  cancelFlow,
  deleteFlow,
  enableFlow,
  getFlowNodeState,
  patchFlowRun,
  resumeFlow,
  runFlow,
  suspendFlow,
  updateFlowNode,
  type Flow,
  type FlowNode,
  type FlowNodeState,
  type NodeDescriptor,
} from "@/lib/flows";
import {
  executedNodeIds,
  flowToEdges,
  flowToNodes,
  nodeStateValues,
  nodesToFlowNodes,
  snapshotColours,
  snapshotValues,
  type FlowCanvasNode,
} from "./flowGraph";
import { FlowNodeView } from "./FlowNodeView";
import { NodeConfigPanel } from "./NodeConfigPanel";
import { Palette } from "./Palette";
import { useFlowRun } from "./useFlowRun";
import { deriveArmedState } from "./armedState";
import { FlowArmedBanner } from "./FlowArmedBanner";

const nodeTypes = { flow: FlowNodeView };

/** How often the canvas re-polls a cron/source flow's runs so a new firing surfaces in the banner
 *  (the count "going up"). A few seconds matches the reactor tick — frequent enough to feel live,
 *  cheap enough (one ws-scoped runs scan). Only an ARMED flow polls. */
const ARMED_REFRESH_MS = 4000;

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

  const { snapshot, error: runError, runs, watch, reattach, refreshRuns } = useFlowRun();

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

  // The PERSISTENT runtime view (Decision 5): every node's current last-value from `flow_node_state`,
  // updated in place each scan. This is the STEADY STATE the canvas paints — the Node-RED "each wire
  // shows its current value" — independent of any single run. Fetched on open + refreshed on the
  // armed tick so a cron flow's values track each firing without reopening.
  const [nodeState, setNodeState] = useState<FlowNodeState | null>(null);
  const loadNodeState = useCallback(async (flowId: string) => {
    try {
      setNodeState(await getFlowNodeState(flowId));
    } catch {
      /* a fresh flow with no state yet — leave null, nodes render blank */
    }
  }, []);
  useEffect(() => {
    void loadNodeState(flow.id);
  }, [flow.id, loadNodeState]);

  // The flow's runtime posture for the banner — armed (headless trigger, enabled) vs idle (manual) vs
  // disabled — plus its latest run. The armed/enabled truth comes from the AUTHORITATIVE node_state
  // (the per-trigger cursors), so it's correct on reload with no run in flight; `runs` drives the
  // "last fired" + count. (deriveArmedState falls back to the flow record until node_state loads.)
  const armed = useMemo(() => deriveArmedState(flow, runs, nodeState), [flow, runs, nodeState]);

  // Deploy/Stop a headless flow by flipping the durable `enabled` flag (`flows.enable`). This is the
  // Stop the user couldn't find for a cron/source flow (which has no live run to cancel) — and because
  // it's durable, the stopped/running state is correct after a server restart. Re-read node_state +
  // runs so the banner flips immediately.
  const handleToggleEnabled = useCallback(async () => {
    const next = !(nodeState?.enabled ?? flow.enabled ?? true);
    setSaveError(null);
    try {
      await enableFlow(flow.id, next);
      await loadNodeState(flow.id);
      await refreshRuns(flow.id);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    }
  }, [flow.id, flow.enabled, nodeState, loadNodeState, refreshRuns]);

  // While ARMED, re-poll the runs list AND the persistent node state on a slow tick so a NEW cron
  // firing surfaces — the banner count + the live per-node values both advance — without reopening
  // the flow. Idle/disabled flows don't poll. Keys on flow.id + armed.kind so it tears down cleanly.
  useEffect(() => {
    if (armed.kind !== "armed") return;
    const t = setInterval(() => {
      void refreshRuns(flow.id);
      void loadNodeState(flow.id);
    }, ARMED_REFRESH_MS);
    return () => clearInterval(t);
  }, [flow.id, armed.kind, refreshRuns, loadNodeState]);

  // A 1s clock so the banner's "next fire in N" / "fired N ago" count down/up live.
  const [nowSecs, setNowSecs] = useState(() => Math.floor(Date.now() / 1000));
  useEffect(() => {
    const t = setInterval(() => setNowSecs(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(t);
  }, []);

  // Paint nodes from the persistent node-state as the BASE (steady-state current values), with the
  // live run snapshot OVERLAID on top while a run is being watched (its in-flight progress + values
  // take precedence for the nodes it touches). So an armed flow with no live run still shows every
  // node's current value, and a run-in-progress shows live deltas — never a frozen "DONE".
  const colours = useMemo(() => (snapshot ? snapshotColours(snapshot) : {}), [snapshot]);
  const values = useMemo(() => {
    const base = nodeState ? nodeStateValues(nodeState) : {};
    const overlay = snapshot ? snapshotValues(snapshot) : {};
    return { ...base, ...overlay };
  }, [nodeState, snapshot]);
  const locked = useMemo(
    () => (snapshot ? executedNodeIds(snapshot) : new Set<string>()),
    [snapshot],
  );
  const runActive = !!snapshot && !isTerminalSnapshot(snapshot);

  // The merged registry keyed by type — used to resolve a node's `kind` (so a trigger renders no
  // target handle) + the selected node's descriptor for the config panel.
  const descriptorByType = useMemo(
    () => new Map(palette.map((d) => [d.type, d])),
    [palette],
  );

  const paintedNodes = useMemo(
    () =>
      nodes.map((n) => {
        const v = values[n.id];
        return {
          ...n,
          data: {
            ...n.data,
            kind: descriptorByType.get(n.data.type)?.kind ?? n.data.kind,
            colour: colours[n.id] ?? "pending",
            locked: locked.has(n.id),
            output: v?.output,
            error: v?.error ?? null,
          },
        };
      }),
    [nodes, colours, values, locked, descriptorByType],
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

  /** Persist JUST the selected node's config (`flows.node.update`) — no whole-flow post
   *  (flow-runtime-control scope). The host validates against the node's descriptor schema and bumps
   *  the flow version; a mismatch surfaces as a 400 inline error. */
  const handleSaveNode = useCallback(async (): Promise<{ ok: boolean; error?: string }> => {
    if (!selectedId) return { ok: false, error: "no selection" };
    try {
      await updateFlowNode(flow.id, selectedId, configs[selectedId] ?? {});
      setPanelError(null);
      return { ok: true };
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setPanelError(msg);
      return { ok: false, error: msg };
    }
  }, [flow.id, selectedId, configs]);

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

  // Import/export the flow JSON (graph + node configs + version). The canonical connection data is
  // each node's `needs` (the record shape the host stores + import re-validates). For legibility we
  // ALSO emit a derived top-level `edges: [{from,to}]` so the connections are visible at a glance in
  // the exported file (the "I can't see the node connections" report) — it is informational; import
  // ignores it and re-derives the graph from `needs`.
  const handleExport = useCallback(() => {
    const exported = buildFlow();
    const edges = exported.nodes.flatMap((n) =>
      (n.needs ?? []).map((from) => ({ from, to: n.id })),
    );
    const blob = new Blob([JSON.stringify({ ...exported, edges }, null, 2)], {
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
  const selectedNode: FlowNode | null = useMemo(() => {
    if (!selectedId) return null;
    return buildFlow().nodes.find((n) => n.id === selectedId) ?? null;
  }, [selectedId, buildFlow]);
  const selectedDescriptor = selectedNode ? descriptorByType.get(selectedNode.type) ?? null : null;

  return (
    <section aria-label="flow canvas" className="flex min-w-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-border bg-card/60 px-3 py-2">
        <Button aria-label="save flow" onClick={handleSave} variant="outline" size="sm" className="gap-1.5">
          <Save size={13} />
          Save
        </Button>
        <Button aria-label="run flow" onClick={handleRun} size="sm" className="gap-1.5">
          <Play size={13} />
          Run
        </Button>
        {runActive ? (
          <>
            <Button aria-label="suspend run" onClick={() => handleLifecycle("suspend")} variant="outline" size="sm" className="gap-1.5">
              <Pause size={13} />
              Suspend
            </Button>
            <Button aria-label="resume run" onClick={() => handleLifecycle("resume")} variant="outline" size="sm" className="gap-1.5">
              <Play size={13} />
              Resume
            </Button>
            <Button aria-label="stop run" onClick={() => handleLifecycle("cancel")} variant="destructive" size="sm" className="gap-1.5">
              <Square size={13} />
              Stop
            </Button>
          </>
        ) : null}
        <div className="mx-1 h-5 w-px bg-border" />
        <Button aria-label="undo" onClick={handleUndo} variant="ghost" size="sm" disabled={undoStack.length === 0} className="gap-1.5">
          <RotateCcw size={13} />
          Undo
        </Button>
        <Button aria-label="export flow" onClick={handleExport} variant="ghost" size="sm" className="gap-1.5">
          <Download size={13} />
          Export
        </Button>
        <Button
          aria-label="import flow"
          onClick={() => importedFile.current?.click()}
          variant="ghost"
          size="sm"
          className="gap-1.5"
        >
          <Upload size={13} />
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
        <div className="ml-auto flex flex-wrap items-center gap-2">
          {snapshot ? (
            <Badge
              variant="outline"
              data-status={snapshot.status}
              className={cn(
                "rounded-full capitalize",
                snapshot.status === "success" && "border-emerald-500/40 text-emerald-600 dark:text-emerald-400",
                (snapshot.status === "failed" || snapshot.status === "partialFailure") &&
                  "border-destructive/40 text-destructive",
                snapshot.status === "running" && "border-amber-500/50 text-amber-600 dark:text-amber-400",
              )}
              aria-label="run status"
            >
              {snapshot.status}
            </Badge>
          ) : null}
          {saveError ? (
            <span aria-label="flow error" className="text-xs text-destructive">
              {saveError}
            </span>
          ) : null}
          {runError ? (
            <span aria-label="run error" className="text-xs text-destructive">
              {runError}
            </span>
          ) : null}
          <Button
            aria-label="delete flow"
            onClick={handleDelete}
            variant="ghost"
            size="sm"
            className="gap-1.5 text-muted hover:text-destructive"
          >
            <Trash2 size={13} />
            Delete
          </Button>
        </div>
      </div>
      <FlowArmedBanner
        armed={armed}
        nowSecs={nowSecs}
        runCount={runs.length}
        onToggle={handleToggleEnabled}
      />
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
            <Background gap={18} size={1} color="hsl(var(--border))" />
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
          onSaveNode={handleSaveNode}
          onPatch={handlePatch}
          onClose={() => setSelectedId(null)}
          error={panelError}
        />
      </div>
    </section>
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
  return (
    s.status === "success" ||
    s.status === "partialFailure" ||
    s.status === "failed" ||
    s.status === "cancelled"
  );
}
