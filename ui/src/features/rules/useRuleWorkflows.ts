// The data hook for the Workflows tab on the rules page (rules-workflow-convergence scope). The
// convergence rule: a flow IS the engine, and "adding a rule to a flow" should be a one-click
// authoring move — the user picks a rule + a trigger, the host stores it as a typed-node flow with
// `trigger → rule` nodes. The user never sees "nodes", "graphs", or "DAG" — only friendly nouns.
//
// This hook owns the read/write surface over `flows.*` (already shipped — no new API/MCP). The
// roster is augmented with the runtime view (`enabled`, `cron`, `nextAttemptTs`) AND the linked rule
// (extracted from the `rule` node's `config.rule`) so the row can paint "rule → trigger → status"
// without a second call. Hydration reads each flow's full record (`getFlow` carries those fields).

import { useCallback, useEffect, useState } from "react";

import {
  deleteFlow,
  enableFlow,
  getFlow,
  listFlows,
  saveFlow,
  type Flow,
  type FlowSummary,
} from "@/lib/flows";

/** A roster row + its hydrated lifecycle + linkage fields. The summary lacks everything but
 *  `{id, name, version, nodes}`; we fetch each flow's full record (small N for a workspace) and
 *  expose the lifecycle fields + the linked rule here. */
export interface WorkflowRow {
  id: string;
  name: string;
  version: number;
  nodeCount: number;
  enabled: boolean;
  cron: string | null | undefined;
  nextAttemptTs: number | undefined;
  trigger: Flow["nodes"][number] | undefined;
  /** The id of the saved rule this workflow runs — extracted from its `rule` node's `config.rule`.
   *  Null when the flow has no `rule` node (a flow that isn't rule-driven). */
  ruleId: string | null;
}

export interface CreateWorkflowInput {
  name: string;
  /** The saved rule to run — becomes a `rule` node wired off the trigger (`config.rule = ruleId`). */
  ruleId: string;
  /** The trigger config — `{mode, cron?, series?, inject_mode?}`. The view's picker owns the shape. */
  triggerConfig: Record<string, unknown>;
  /** Start enabled (the default — most authors expect the flow armed the moment they save). */
  enabled?: boolean;
}

export interface RuleWorkflowsState {
  rows: WorkflowRow[];
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  create: (input: CreateWorkflowInput) => Promise<{ ok: boolean; id?: string; error?: string }>;
  toggle: (id: string, enabled: boolean) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

/** Find the trigger node on a saved flow. The seed every blank flow ships with is the node of
 *  `type: "trigger"`; fall back to the first node for hand-built graphs that lack one. */
function pickTrigger(flow: Flow): Flow["nodes"][number] | undefined {
  return flow.nodes.find((n) => n.type === "trigger") ?? flow.nodes[0];
}

/** Extract the saved-rule id this flow runs, if any. A flow that came from the Workflows tab carries
 *  one `rule` node with `config.rule = "<id>"`. A flow without that node isn't rule-driven. */
function pickRuleId(flow: Flow): string | null {
  for (const n of flow.nodes) {
    if (n.type === "rule") {
      const id = n.config?.rule;
      if (typeof id === "string" && id.length > 0) return id;
    }
  }
  return null;
}

function toRow(flow: Flow): WorkflowRow {
  return {
    id: flow.id,
    name: flow.name,
    version: flow.version,
    nodeCount: flow.nodes.length,
    enabled: flow.enabled ?? false,
    cron: flow.cron,
    nextAttemptTs: flow.nextAttemptTs,
    trigger: pickTrigger(flow),
    ruleId: pickRuleId(flow),
  };
}

export function useRuleWorkflows(ws: string): RuleWorkflowsState {
  const [rows, setRows] = useState<WorkflowRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const summaries: FlowSummary[] = await listFlows();
      // Hydrate each row from its saved record — `getFlow` carries lifecycle + node-config fields.
      const flows = await Promise.all(summaries.map((s) => getFlow(s.id).catch(() => null)));
      setRows(
        flows
          .filter((f): f is Flow => f !== null)
          .map(toRow)
          // Rule-driven workflows first (the Workflows tab created them); others after. Within each
          // group, newest first so an author's just-created workflow lands at the top.
          .sort((a, b) => {
            if (!!a.ruleId !== !!b.ruleId) return a.ruleId ? -1 : 1;
            return a.id < b.id ? 1 : -1;
          }),
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    // Refetch on focus so a workflow touched in another tab (the flows canvas, a dashboard control)
    // re-paints its current enabled/next-fire without a manual reload.
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [ws, refresh]);

  const create = useCallback(
    async (input: CreateWorkflowInput): Promise<{ ok: boolean; id?: string; error?: string }> => {
      try {
        const id = `flow-${Date.now()}`;
        const flow: Flow = {
          id,
          name: input.name.trim() || "Untitled workflow",
          version: 1,
          enabled: input.enabled ?? true,
          nodes: [
            { id: "start", type: "trigger", needs: [], config: input.triggerConfig },
            // The rule runs off the trigger — `needs: ["start"]` is the DAG edge. `config.rule` is the
            // saved-rule id the `rule` node dispatches via `rules.eval` (`{rule_id, params}`).
            { id: "run-rule", type: "rule", needs: ["start"], config: { rule: input.ruleId } },
          ],
          failurePolicy: "halt",
        };
        await saveFlow(flow);
        await refresh();
        return { ok: true, id };
      } catch (e) {
        return { ok: false, error: e instanceof Error ? e.message : String(e) };
      }
    },
    [refresh],
  );

  const toggle = useCallback(
    async (id: string, enabled: boolean) => {
      // Optimistic flip so the switch settles instantly; the host is the source of truth and a
      // failure reverts on the next `refresh`.
      setRows((cur) => cur.map((r) => (r.id === id ? { ...r, enabled } : r)));
      try {
        await enableFlow(id, enabled);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        await refresh();
      }
    },
    [refresh],
  );

  const remove = useCallback(
    async (id: string) => {
      try {
        await deleteFlow(id);
        setRows((cur) => cur.filter((r) => r.id !== id));
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
        await refresh();
      }
    },
    [refresh],
  );

  return { rows, loading, error, refresh, create, toggle, remove };
}
