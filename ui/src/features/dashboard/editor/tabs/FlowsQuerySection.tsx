// The Flows datasource section of the Query tab (flow-dashboard-binding-ux-scope) — the flow→node→port
// picker, surfaced inside the ONE PanelEditor (so add == edit, one editor). It reads the shipped Flows
// source-picker entries (built from `flows.list`/`flows.get`/`flows.nodes`) and, on a port selection,
// rewrites the editor state:
//   - an INPUT port  → a write `action` (`flows.inject`, port-aware) + a control view (slider default);
//   - an OUTPUT port → a read `source` (`flows.node_state`) + a read view (jsonview default).
// The author picks from friendly labels (`cooler-ctl › setpoint-in › payload (input)`) — never a tool
// name. Selecting a port that no longer exists renders an honest "re-pick" state, never a silent no-op.

import { useMemo } from "react";

import type { FieldConfig, Target, View } from "@/lib/dashboard";
import type { EditorState } from "../cellEditorState";
import type { SourceEntry } from "../../builder/sourcePicker";
import { flowBindingOfAction, flowBindingOfSource } from "../../views/flowBinding";
import { asPath, type PathSeg } from "../../views/jsonPaths";
import { JsonPathPicker } from "./JsonPathPicker";

const FIELD =
  "h-8 rounded-md border border-border bg-bg px-2.5 text-xs text-fg focus-visible:border-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/20";

interface Props {
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  /** All source-picker entries (the Flows group is the `group === "flows"` subset). */
  entries: SourceEntry[];
  loading: boolean;
}

/** The id the editor currently binds (input via the action, output via the source) → match a Flows entry. */
function selectedFlowEntryId(state: EditorState, entries: SourceEntry[]): string | null {
  const action = state.carry.action;
  const inBind = flowBindingOfAction(action);
  if (inBind) {
    const e = entries.find(
      (x) => x.id === `flows:in:${inBind.flowId}:${inBind.node}:${inBind.port}`,
    );
    return e?.id ?? null;
  }
  const src = state.targets[0];
  const outBind = src
    ? flowBindingOfSource({ tool: src.tool, args: src.args as Record<string, unknown> })
    : null;
  if (outBind) {
    const e = entries.find(
      (x) => x.id === `flows:out:${outBind.flowId}:${outBind.node}:${outBind.port}`,
    );
    return e?.id ?? null;
  }
  return null;
}

export function FlowsQuerySection({ state, patch, entries, loading }: Props) {
  const flowEntries = useMemo(() => entries.filter((e) => e.group === "flows"), [entries]);
  const selectedId = selectedFlowEntryId(state, entries);
  // Does the editor currently HOLD a flow binding (input action or output source)?
  const hasFlowBinding = Boolean(
    flowBindingOfAction(state.carry.action) ||
      (state.targets[0] &&
        flowBindingOfSource({
          tool: state.targets[0].tool,
          args: state.targets[0].args as Record<string, unknown>,
        })),
  );
  // "Broken" means: a binding is held, the picker FINISHED loading with real entries, and the bound
  // flow/node/port is genuinely not among them (renamed/deleted). Never flash "broken" while the
  // entries are still loading or empty (the async `useSourcePicker` race that mis-fired this hint).
  const broken = hasFlowBinding && !loading && flowEntries.length > 0 && !selectedId;

  // The active OUTPUT binding (a `flows.node_state` source) — drives the JSON path builder below.
  const outTarget = state.targets[0];
  const outBind =
    outTarget?.tool === "flows.node_state"
      ? flowBindingOfSource({ tool: outTarget.tool, args: outTarget.args as Record<string, unknown> })
      : null;
  const boundPath = asPath((outTarget?.args as Record<string, unknown> | undefined)?.__flowPath);

  /** Write a chosen JSON path onto the output source args (the read view extracts exactly this path).
   *  When the picked leaf LOOKS like a flow timestamp (`ts` / `*_ts` / `*_at`) and the author hasn't set
   *  a field unit yet, SUGGEST the flow-seconds datetime unit so a stat/gauge cell renders it as the
   *  viewer's wall-clock (flow-ts-display scope). Only a default — the field tab's unit picker overrides.
   */
  const pickPath = (path: PathSeg[]) => {
    if (!outTarget) return;
    const next: Partial<EditorState> = {
      targets: [{ ...outTarget, args: { ...(outTarget.args as Record<string, unknown>), __flowPath: path } }],
    };
    const suggested = suggestTimestampUnit(path, state.fieldConfig);
    if (suggested) next.fieldConfig = suggested;
    patch(next);
  };

  const selectPort = (id: string) => {
    const entry = flowEntries.find((e) => e.id === id);
    if (!entry) {
      // "— pick a port —" → clear the flow binding (leave the cell target-less).
      patch({
        targets: [{ refId: state.targets[0]?.refId || "A", tool: "", args: {}, datasource: { type: "flows" } }],
        carry: { ...state.carry, action: undefined },
      });
      return;
    }
    if (entry.writes && entry.action) {
      // INPUT port → a write control. The action drives the inject; default to a slider view.
      const view: View = controlViewFor(state.view) ?? "slider";
      patch({
        targets: [{ refId: state.targets[0]?.refId || "A", tool: "", args: {}, datasource: { type: "flows" } }],
        carry: { ...state.carry, action: entry.action },
        view,
      });
    } else if (entry.source) {
      // OUTPUT port → a read view over `flows.node_state`; default to the JSON/object view. Default the
      // JSON path to the port's own field (e.g. `[payload]`) so the picker shows it selected and the
      // rendered widget matches the preview immediately — the user can drill deeper or pick "(whole
      // value)" to override. The node's recorded value is the `{payload, …}` envelope; the port name is
      // its top-level key.
      const srcArgs = (entry.source.args as Record<string, unknown>) ?? {};
      const port = (srcArgs.__flowPort as string | undefined) ?? "payload";
      const target: Target = {
        refId: state.targets[0]?.refId || "A",
        tool: entry.source.tool,
        args: { ...srcArgs, __flowPath: [port] },
        datasource: { type: "flows" },
      };
      patch({
        targets: [target],
        carry: { ...state.carry, action: undefined },
        view: "jsonview",
      });
    }
  };

  return (
    <div className="grid gap-2" aria-label="flows query section">
      <label className="grid gap-1 text-xs text-muted">
        Flow node port
        {/* eslint-disable-next-line no-restricted-syntax -- no shadcn Select primitive yet (dashboard.md follow-up) */}
        <select
          aria-label="flow node port"
          className={`${FIELD} w-full`}
          value={selectedId ?? ""}
          disabled={loading}
          onChange={(e) => selectPort(e.target.value)}
        >
          <option value="">— pick a flow node + port —</option>
          {flowEntries.map((e) => (
            <option key={e.id} value={e.id}>
              {e.label}
            </option>
          ))}
        </select>
      </label>
      {broken && (
        <p className="text-xs text-danger" role="status">
          binding broken — re-pick a flow node port
        </p>
      )}
      {!loading && flowEntries.length === 0 && (
        <p className="text-xs text-muted" role="status">
          no flows you can read in this workspace
        </p>
      )}

      {/* The visual JSON builder — once an OUTPUT node is bound, introspect its live value and click a
          field to bind exactly that path (object / array / nested). The read view shows just that. */}
      {outBind && (
        <JsonPathPicker
          flowId={outBind.flowId}
          node={outBind.node}
          path={boundPath}
          onPick={pickPath}
        />
      )}
    </div>
  );
}

/** Keep an already-chosen control view when re-picking an input port (slider/switch/json); otherwise
 *  default to a slider. A non-control current view (a viz) falls through to the slider default. */
function controlViewFor(view: View | ""): View | null {
  return view === "switch" || view === "slider" || view === "json" ? view : null;
}

/** A leaf whose NAME reads like an epoch timestamp — `ts`, `*_ts`, `*_at` (created_at, updated_at, …).
 *  The heuristic is deliberately narrow so a plain numeric field is never mis-formatted as a date. */
function looksLikeTimestamp(path: PathSeg[]): boolean {
  const leaf = path[path.length - 1];
  if (typeof leaf !== "string") return false; // an array index is never a timestamp name
  return leaf === "ts" || leaf.endsWith("_ts") || leaf.endsWith("_at");
}

/** Suggest the flow-seconds datetime unit for a timestamp-looking leaf — ONLY when the author hasn't
 *  already set a field unit (never clobber an explicit choice). Returns the patched `fieldConfig`, or
 *  `null` to leave it untouched. The flow clock is epoch SECONDS, so the `time:flow-seconds` mapping
 *  (with `dateUnit:"s"`) is the right default; the field tab's unit picker can override to anything.
 *  Exported for the unit test (pure logic, no render). */
export function suggestTimestampUnit(path: PathSeg[], current: FieldConfig | undefined): FieldConfig | null {
  if (!looksLikeTimestamp(path)) return null;
  if (current?.defaults?.unit) return null; // author already chose a unit — respect it
  const base = current ?? { defaults: {}, overrides: [] };
  return { ...base, defaults: { ...base.defaults, unit: "time:flow-seconds" } };
}
