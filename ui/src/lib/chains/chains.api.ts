// The chains API client — one call per export, mirroring the gateway's `chains.*` routes and the host
// verbs 1:1 (rules-workbench scope, Phase 2). The UI never calls `invoke` directly; it goes through
// these named verbs (FILE-LAYOUT frontend rules). Each is capability-gated server-side; the workspace
// + principal come from the session token (the hard wall, §7), never an argument.

import type { Chain, ChainSummary, RunSnapshot } from "./chains.types";
import { invoke } from "@/lib/ipc/invoke";

/** The chains the caller can reach in the workspace. Mirrors `chains.list`. */
export function listChains(): Promise<ChainSummary[]> {
  return invoke<{ chains: ChainSummary[] }>("chains_list", {}).then((r) => r.chains);
}

/** Read one chain (its DAG). Mirrors `chains.get`. */
export function getChain(id: string): Promise<Chain> {
  return invoke<Chain>("chains_get", { id });
}

/** Create/update a chain (DAG-validated UPSERT on `id`). The workspace is set host-side from the
 *  token. Mirrors `chains.save`; an invalid DAG rejects with the host's `400` validation message (the
 *  canvas inline error). Returns `{ id }`. */
export function saveChain(chain: Chain): Promise<{ id: string }> {
  return invoke<{ id: string }>("chains_save", { chain });
}

/** Soft-delete a chain (idempotent tombstone). Mirrors `chains.delete`. */
export function deleteChain(id: string): Promise<void> {
  return invoke<void>("chains_delete", { id });
}

/** Start a chain run (a durable job). Mirrors `chains.run`; returns the `run_id` the canvas polls. */
export function runChain(id: string, params?: Record<string, unknown>): Promise<{ run_id: string }> {
  return invoke<{ run_id: string }>("chains_run", { id, params: params ?? {} });
}

/** Read a run's per-step snapshot (the canvas's settle-colouring source). Mirrors `chains.runs.get`. */
export function getChainRun(id: string, runId: string): Promise<RunSnapshot> {
  return invoke<RunSnapshot>("chains_run_get", { id, runId });
}
