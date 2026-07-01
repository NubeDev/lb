// The flows API client — one call per export, mirroring the gateway's `flows.*` routes and the host
// verbs 1:1 (flows-canvas scope, Wave 3). The UI never calls `invoke` directly; it goes through these
// named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the workspace +
// principal come from the session token (the hard wall, §7), never an argument. `flows.inject` is the
// one write tool a dashboard control calls (dashboard-binding scope) — re-checked per call like any
// control write.

import { invoke } from "@/lib/ipc/invoke";
import type {
  Flow,
  FlowNodeState,
  FlowRunSnapshot,
  FlowRunSummary,
  FlowSummary,
  InjectFlowResult,
  NodeDescriptor,
  RunFlowResult,
  SaveFlowResult,
} from "./flows.types";

/** The merged node registry (the palette source). Mirrors `flows.nodes` (built-ins ∪ installed-ext
 *  nodes). The descriptor declares no caps — the deny lives at run time. */
export function listFlowNodes(): Promise<NodeDescriptor[]> {
  return invoke<{ nodes: NodeDescriptor[] }>("flows_nodes", {}).then((r) => r.nodes);
}

/** The flows the caller can reach in the workspace. Mirrors `flows.list`. */
export function listFlows(): Promise<FlowSummary[]> {
  return invoke<{ flows: FlowSummary[] }>("flows_list", {}).then((r) => r.flows);
}

/** Read one flow (its typed graph). Mirrors `flows.get`. */
export function getFlow(id: string): Promise<Flow> {
  return invoke<Flow>("flows_get", { id });
}

/** Create/update a flow (DAG + every node config validated UPSERT on `id`). The workspace is set
 *  host-side from the token. Mirrors `flows.save`; an invalid DAG or schema-invalid node config
 *  rejects with the host's `400` validation message (the canvas inline error). Returns
 *  `{ id, version }`. */
export function saveFlow(flow: Flow): Promise<SaveFlowResult> {
  return invoke<SaveFlowResult>("flows_save", { flow });
}

/** Guarded, ordered teardown (disarm sources, cancel runs, drop cron). Mirrors `flows.delete`. */
export function deleteFlow(id: string): Promise<void> {
  return invoke<void>("flows_delete", { id });
}

/** Start a flow run (a durable job). Mirrors `flows.run`; returns the `run_id` the canvas polls. */
export function runFlow(id: string, params?: Record<string, unknown>): Promise<RunFlowResult> {
  return invoke<RunFlowResult>("flows_run", { id, params: params ?? {} });
}

/** Suspend a run (the coordinator stops enqueuing the next frontier). Mirrors `flows.suspend`. */
export function suspendFlow(runId: string): Promise<void> {
  return invoke<void>("flows_suspend", { runId });
}

/** Resume a suspended/interrupted run (idempotent — the CAS step-claim owns exactly-once). Mirrors
 *  `flows.resume`. */
export function resumeFlow(runId: string): Promise<void> {
  return invoke<void>("flows_resume", { runId });
}

/** Cancel a run. Mirrors `flows.cancel`. */
export function cancelFlow(runId: string): Promise<void> {
  return invoke<void>("flows_cancel", { runId });
}

/** A config-only patch to an UNEXECUTED node of a live run, validated against the run's PINNED schema
 *  (Decision 12). Mirrors `flows.patch_run`; a patch to an executed node or a schema mismatch rejects
 *  with the host's `400`. */
export function patchFlowRun(
  runId: string,
  node: string,
  config: Record<string, unknown>,
): Promise<void> {
  return invoke<void>("flows_patch_run", { runId, node, config });
}

/** Read a run's per-node snapshot (the canvas poll + the executed-node-lock surface). Mirrors
 *  `flows.runs.get`. */
export function getFlowRun(runId: string): Promise<FlowRunSnapshot> {
  return invoke<FlowRunSnapshot>("flows_run_get", { runId });
}

/** Read one node's config from the SAVED flow (flow-runtime-control-scope). Mirrors `flows.node.get`.
 *  Returns the node's `{id, type, config}`. */
export function getFlowNode(
  id: string,
  node: string,
): Promise<{ id: string; type: string; config: Record<string, unknown> }> {
  return invoke<{ id: string; type: string; config: Record<string, unknown> }>("flows_node_get", {
    id,
    node,
  });
}

/** Replace one node's config on the SAVED flow without re-posting the whole `Flow`
 *  (flow-runtime-control-scope). Validates against the node's descriptor schema (a mismatch rejects
 *  with the host's `400`) and bumps the flow version. Mirrors `flows.node.update`. Returns the new
 *  `{id, node, version}`. */
export function updateFlowNode(
  id: string,
  node: string,
  config: Record<string, unknown>,
): Promise<{ id: string; node: string; version: number }> {
  return invoke<{ id: string; node: string; version: number }>("flows_node_update", {
    id,
    node,
    config,
  });
}

/** The runs of a flow (the reattach surface: a reopened canvas finds the active `run_id`). Mirrors
 *  `flows.runs.list`. Pass `status:"active"` to find an in-flight run. */
export function listFlowRuns(
  flowId: string,
  status?: string,
): Promise<FlowRunSummary[]> {
  return invoke<{ runs: FlowRunSummary[] }>("flows_runs_list", {
    flowId,
    status: status ?? null,
  }).then((r) => r.runs);
}

/** The persistent runtime view — every node's CURRENT last-value + the flow's armed fields (Decision
 *  5). The canvas paints this as its steady state (independent of any single run). Mirrors
 *  `flows.node_state`. */
export function getFlowNodeState(id: string): Promise<FlowNodeState> {
  return invoke<FlowNodeState>("flows_node_state", { id });
}

/** Flip the durable lifecycle flags (enable/disable + start_on_boot). Mirrors `flows.enable`. */
export function enableFlow(
  id: string,
  enabled: boolean,
  startOnBoot = false,
): Promise<void> {
  return invoke<void>("flows_enable", { id, enabled, startOnBoot });
}

/** Set a node's retained value (Decision 9) — the write tool a dashboard control calls. Sets a
 *  retained input (no run) OR fires a one-shot run for a firing trigger. Mirrors `flows.inject`;
 *  re-checked per call (`caller ∩ grant`, workspace from the token). Returns whether a run fired. */
export function injectFlow(
  id: string,
  node: string,
  value: unknown,
  port?: string,
): Promise<InjectFlowResult> {
  return invoke<InjectFlowResult>("flows_inject", { id, node, value, port });
}
