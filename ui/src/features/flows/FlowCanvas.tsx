// The typed-node DAG canvas (flows-canvas scope, Wave 3). React Flow renders the open flow: nodes =
// typed graph nodes, edges = `needs`. An author drags node types from the palette onto the canvas,
// wires them, and configures each via the schema-rendered side panel. Save calls `flows.save` — a
// cyclic/invalid DAG or schema-invalid node config renders the host's `400` message INLINE (no
// crash). Run calls `flows.run` then the canvas polls `flows.runs.get` (bounded) and paints each node
// as it settles, with suspend/resume/cancel + the executed-node-lock + `flows.patch_run` for
// unexecuted nodes (Decision 1/12). Import/export round-trips the flow JSON; undo restores a prior
// graph (node + edges) atomically by re-saving the previous version.

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

import {
  cancelFlow,
  deleteFlow,
  enableFlow,
  patchFlowRun,
  resumeFlow,
  runFlow,
  suspendFlow,
  updateFlowNode,
  type Flow,
  type FlowNode,
  type NodeDescriptor,
} from "@/lib/flows";
import {
  flowToEdges,
  flowToNodes,
  isTerminalStatus,
  lockedNodeIds,
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
import { deriveRuntimeState } from "./runtimeState";
import { FlowRuntimeBanner } from "./FlowRuntimeBanner";
import { FlowCanvasHeader } from "./FlowCanvasHeader";
import { flowDirty } from "./flowDirty";
import { useLiveValues } from "./useLiveValues";
import { downloadFlow, parseImportedFlow } from "./flowTransfer";
import { DebugPanel } from "./debug/DebugPanel";
import { defaultConfig } from "./defaultConfig";

const nodeTypes = { flow: FlowNodeView };

/** How often the canvas re-polls a RUNNING flow's node-state + runs so live values (and the count
 *  "going up") surface without reopening. A few seconds matches the reactor tick — frequent enough to
 *  feel live, cheap enough (one ws-scoped read). Any enabled flow polls while live values are on. */
const RUNTIME_REFRESH_MS = 4000;

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
  // The debug panel drawer (debug-node-scope). Toggled by the Bug button in the header; tails the
  // flow's `debug` nodes over the SSE route. Auto-opens when a debug node is present in the flow on
  // open (Node-RED shows the sidebar when there's something to debug).
  const [debugOpen, setDebugOpen] = useState(false);
  // The DEPLOYED graph — what the running system currently holds. The canvas is a draft; Deploy is
  // enabled only when the buffer differs from this (Node-RED posture). Updated on every successful
  // Deploy/per-node Save so the dirty flag clears (flow-deploy-ux scope).
  const [deployedFlow, setDeployedFlow] = useState<Flow>(flow);
  const { snapshot, error: runError, runs, watch, reattach, refreshRuns, markTerminal } =
    useFlowRun();

  // Re-seed the canvas when a different flow opens — a faithful load + reattach to an active run.
  useEffect(() => {
    setNodes(flowToNodes(flow));
    setEdges(flowToEdges(flow));
    setConfigs(indexConfigs(flow));
    setSelectedId(null);
    setSaveError(null);
    setPanelError(null);
    setUndoStack([]);
    setDeployedFlow(flow); // a freshly-opened flow IS the deployed graph → Deploy starts clean.
    // Auto-open the debug panel when the flow has a debug node (Node-RED shows the sidebar when
    // there's something to debug). The operator can still close it from the header toggle.
    setDebugOpen(flow.nodes.some((n) => n.type === "debug"));
    void reattach(flow.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [flow.id]);

  // The PERSISTENT runtime view + the live-values toggle + its polling live in the hook (Decision 5;
  // the Node-RED "each wire shows its current value"). It re-polls whenever the flow is RUNNING so a
  // live runtime's values advance without reopening.
  const { nodeState, liveValues, loadNodeState, toggleLiveValues } = useLiveValues(
    flow.id,
    refreshRuns,
  );

  // The flow's runtime posture for the banner — running (enabled) vs stopped (disabled) — plus its
  // latest run. `enabled` comes from the AUTHORITATIVE node_state (the per-trigger cursors), so it's
  // correct on reload with no run in flight; `runs` drives the "last fired" + count.
  const runtime = useMemo(() => deriveRuntimeState(flow, runs, nodeState), [flow, runs, nodeState]);
  const enabled = runtime.running;

  // Enable/Disable the flow by flipping the durable `enabled` flag (`flows.enable`) — "should this ever
  // fire" (durable, survives restart). Re-read node_state + runs so the banner flips immediately.
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

  // While the flow is RUNNING + watching live values, re-poll runs + node-state on a slow tick so a
  // live runtime's values + count advance without reopening — regardless of what drives it (cron,
  // flipflop, or manual runs). Stopped/off → no interval (tears down cleanly). This keys off `running`,
  // NOT the graph shape: a self-driving flow the old "armed" guess didn't recognise (e.g. a flipflop)
  // used to freeze here even though the host was advancing it every second.
  useEffect(() => {
    if (!enabled || !liveValues) return;
    const t = setInterval(() => {
      void refreshRuns(flow.id);
      void loadNodeState(flow.id);
    }, RUNTIME_REFRESH_MS);
    return () => clearInterval(t);
  }, [flow.id, enabled, liveValues, refreshRuns, loadNodeState]);

  // A 1s clock so the banner's "next fire in N" / "fired N ago" count down/up live.
  const [nowSecs, setNowSecs] = useState(() => Math.floor(Date.now() / 1000));
  useEffect(() => {
    const t = setInterval(() => setNowSecs(Math.floor(Date.now() / 1000)), 1000);
    return () => clearInterval(t);
  }, []);

  const runActive = !!snapshot && !isTerminalStatus(snapshot.status);
  // Paint nodes from the persistent node-state (the live steady-state values). While a run is genuinely
  // IN FLIGHT, overlay that run's in-progress values/colours on top so the operator sees live deltas.
  // A TERMINAL snapshot must NOT overlay — otherwise a finished run's frozen "DONE" values would mask
  // the advancing steady-state (the reported "stuck on 26 / both DONE" freeze). node_state is always
  // the source of truth once a run settles.
  const colours = useMemo(() => (runActive ? snapshotColours(snapshot) : {}), [runActive, snapshot]);
  const values = useMemo(() => {
    const base = nodeState ? nodeStateValues(nodeState) : {};
    const overlay = runActive && snapshot ? snapshotValues(snapshot) : {};
    return { ...base, ...overlay };
  }, [nodeState, runActive, snapshot]);
  // The executed-node lock — gated on a genuinely in-flight run by `lockedNodeIds`. A terminal
  // snapshot locks nothing, so the operator edits without a page refresh. (See lockedNodeIds.)
  const locked = useMemo(() => lockedNodeIds(snapshot), [snapshot]);

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
      // Seed the per-node starter config (a freshly-added `rhai` node, for example, opens with a
      // working payload-routing template instead of a blank source box).
      setConfigs((c) => ({ ...c, [id]: defaultConfig(desc.type) }));
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

  // Dirty = the canvas buffer differs from the DEPLOYED graph → Deploy is enabled (Node-RED posture).
  // Cleared on a successful Deploy/per-node Save (which advance `deployedFlow`). See flowDirty.ts.
  const dirty = useMemo(() => flowDirty(deployedFlow, buildFlow()), [deployedFlow, buildFlow]);

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
      // Deploy landed: the buffer IS the deployed graph now → Deploy goes clean. Carry the host's new
      // version so the compare (which ignores version) and the roster stay consistent.
      setDeployedFlow({ ...next, version: res.version ?? next.version });
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
      // Open the live stream only when the operator is watching values; otherwise the run still drives
      // headless (fire-and-forget) and the terminal state is read on the next refresh.
      if (liveValues) watch(run_id);
    } catch (e) {
      setSaveError(e instanceof Error ? e.message : String(e));
    }
  }, [flow.id, watch, liveValues]);

  const handleLifecycle = useCallback(
    async (op: "suspend" | "resume" | "cancel") => {
      if (!snapshot) return;
      try {
        if (op === "suspend") await suspendFlow(snapshot.runId);
        if (op === "resume") await resumeFlow(snapshot.runId);
        if (op === "cancel") {
          await cancelFlow(snapshot.runId);
          // Release the run-active lock NOW so the operator can edit nodes immediately — don't wait on
          // the SSE `run-finished` frame (it may never arrive if the stream already closed, which is
          // exactly what forced a page refresh before). The host is re-read as the source of truth on
          // the next open / refresh.
          markTerminal("cancelled");
        }
      } catch (e) {
        setSaveError(e instanceof Error ? e.message : String(e));
      }
    },
    [snapshot, markTerminal],
  );

  /** Persist JUST the selected node's config (`flows.node.update`) — no whole-flow post
   *  (flow-runtime-control scope). The host validates against the node's descriptor schema and bumps
   *  the flow version; a mismatch surfaces as a 400 inline error. */
  const handleSaveNode = useCallback(async (): Promise<{ ok: boolean; error?: string }> => {
    if (!selectedId) return { ok: false, error: "no selection" };
    try {
      await updateFlowNode(flow.id, selectedId, configs[selectedId] ?? {});
      setPanelError(null);
      // The node's config is deployed now → clear just that node's dirtiness by advancing the deployed
      // graph's copy of it (a whole-graph Deploy isn't needed for a single-node tweak).
      setDeployedFlow((d) => ({
        ...d,
        nodes: d.nodes.map((n) =>
          n.id === selectedId ? { ...n, config: configs[selectedId] ?? {} } : n,
        ),
      }));
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
  const handleExport = useCallback(() => downloadFlow(buildFlow()), [buildFlow]);

  const handleImport = useCallback(
    async (file: File) => {
      try {
        // Re-validate the imported graph through the real save path (schema + DAG). Surfaces a 400
        // inline on mismatch; on success the parent reopens the flow (re-seeding the canvas + deployed).
        const next = await parseImportedFlow(file, flow);
        const res = await onSave(next);
        if (!res.ok) setSaveError(res.error ?? "import failed");
      } catch (e) {
        setSaveError(e instanceof Error ? e.message : String(e));
      }
    },
    [flow, onSave],
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
      <FlowCanvasHeader
        dirty={dirty}
        runActive={runActive}
        enabled={enabled}
        liveValues={liveValues}
        onDeploy={handleSave}
        onRun={handleRun}
        onLifecycle={handleLifecycle}
        onToggleEnabled={handleToggleEnabled}
        onToggleLiveValues={toggleLiveValues}
        canUndo={undoStack.length > 0}
        runStatus={snapshot?.status ?? null}
        saveError={saveError}
        runError={runError}
        debugOpen={debugOpen}
        onToggleDebug={() => setDebugOpen((o) => !o)}
        onUndo={handleUndo}
        onExport={handleExport}
        onImport={(f) => void handleImport(f)}
        onDelete={handleDelete}
      />
      <FlowRuntimeBanner runtime={runtime} nowSecs={nowSecs} runCount={runs.length} />
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
        {debugOpen ? (
          <DebugPanel flowId={flow.id} onClose={() => setDebugOpen(false)} />
        ) : null}
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
