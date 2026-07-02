// `GenUiView` — the `view:"genui"` dashboard tenant of `@nube/genui` (genui-scope "The widget"). It
// mounts the reusable `<GenUiSurface>` over the cell's persisted IR (`options.genui.ir`) and feeds it a
// `/data/{refId}` data model derived from the cell's v3 `sources[]`, each resolved through the SAME
// shipped `usePanelData` path every other view uses — so source-picker, variables, watch/refresh
// cadence, and the deny/empty states are all inherited, not re-implemented.
//
// TRUST TIER — IN-PROCESS (genui-scope Decision 1, AMENDED). The scope originally said "sandboxed
// iframe", but the shipped `WidgetIframe` sandbox cannot host a React surface (no import map for bare
// `react`, CSP `connect-src 'none'`, eval'd non-React engines — see
// debugging/frontend/ext-widget-iframe-tier-cannot-resolve-bare-react.md). A catalog IR is trusted
// DATA rendered by OUR components (not author code), genui widgets are admin-authored (the
// `dashboard.save` cap is the trust gate, like an installed extension widget), and the catalog satisfies
// all five promotion-checklist items (CI-tested). So the surface renders in-process — the promotion
// end-state — avoiding the double-React / per-tick-postMessage tax and keeping offline rendering.
//
// Actions still go through the LEASHED bridge: a control's `emit` maps to `bridge.call(tool, args)`
// where the bridge is `makeWidgetBridge(cellTools(cell))` — rejected locally if outside the leash and
// re-capability-checked + workspace-checked at the host per call. The token never enters this layer.

import { useEffect, useMemo, useState } from "react";
import { GenUiSurface, migrate, nubeCatalog, type IrSpec, type UiAction } from "@nube/genui";
import "@nube/genui/style.css";

import type { Cell, Target } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { WidgetHeader, WidgetMessage } from "../../widgets/chrome";
import { makeWidgetBridge } from "../../builder/widgetBridge";
import { usePanelData } from "../../builder/usePanelData";
import { cellTools } from "../WidgetView";
import {
  genuiTargets,
  refDataOf,
  singleTargetCell,
  type GenUiDataModel,
  type RefData,
} from "./genuiData";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

/** Read the persisted, versioned IR off the cell. Three outcomes:
 *   - `{ ir }`     — a well-formed IR (migrated forward) to render;
 *   - `"draft"`    — an UN-AUTHORED cell (no `genui` block, or one with no `ir`): the author just added
 *                    an "AI widget" and hasn't generated it yet — show an author-me placeholder, NOT an
 *                    error (the host accepts this as a savable draft too);
 *   - `"invalid"`  — a `genui` block with a present-but-broken `ir` (defense-in-depth; the host rejects
 *                    these on save, but a hand-built/legacy cell degrades here honestly, never a crash). */
function cellIr(cell: Cell): { ir: IrSpec } | "draft" | "invalid" {
  const g = (cell.options as Record<string, unknown> | undefined)?.genui as
    | { v?: number; ir?: unknown }
    | undefined;
  if (!g || g.ir == null) return "draft"; // no block, or block without an IR yet
  if (typeof g.ir !== "object") return "invalid";
  return { ir: migrate(g.ir as IrSpec) };
}

/** One data probe per target: it calls `usePanelData` (the hook can only run in a component) on a
 *  one-source synthetic cell and reports the resolved `RefData` up by refId. The probe list is stable
 *  for a given cell (its `sources[]`), so the hook count is stable across renders (rules-of-hooks). */
function TargetProbe({
  parent,
  target,
  scope,
  refreshKey,
  onData,
}: {
  parent: Cell;
  target: Target;
  scope: VarScope;
  refreshKey: number;
  onData: (refId: string, data: RefData) => void;
}) {
  const cell = useMemo(() => singleTargetCell(parent, target), [parent, target]);
  const state = usePanelData(cell, scope, refreshKey);
  const data = refDataOf(state);
  // Report to the parent AFTER commit (never during render — a setState-in-render on the parent). Keyed
  // on the content signature so an identical resolve doesn't re-report (usePanelData returns a fresh
  // object each render).
  const sig = JSON.stringify(data);
  useEffect(() => {
    onData(target.refId, data);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `sig` is the content signature of `data`
  }, [target.refId, sig]);
  return null;
}

export function GenUiView({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const parsed = cellIr(cell);
  const ir = typeof parsed === "object" ? parsed.ir : null;
  const targets = useMemo(() => genuiTargets(cell), [cell]);
  const tools = cellTools(cell);
  const toolsKey = tools.join("|");
  // eslint-disable-next-line react-hooks/exhaustive-deps -- re-create the bridge only when the tool SET changes
  const bridge = useMemo(() => makeWidgetBridge(tools), [toolsKey]);

  // The per-refId data model, collected from the probes. A plain object in state; a probe overwrites its
  // refId entry when its resolve changes.
  const [byRef, setByRef] = useState<Record<string, RefData>>({});
  const onData = useMemo(() => {
    return (refId: string, data: RefData) =>
      setByRef((prev) => (JSON.stringify(prev[refId]) === JSON.stringify(data) ? prev : { ...prev, [refId]: data }));
  }, []);

  const model: GenUiDataModel = useMemo(() => ({ data: byRef }), [byRef]);

  // A control action → a leashed, host-re-checked write. `emit` gives us `{tool, context}`; the tool is
  // whatever the catalog control named (bound on the cell), the context the resolved args. The bridge
  // rejects anything outside `cellTools`; the host re-checks the cap + workspace.
  const onAction = (a: UiAction) => {
    const toolName = a.tool;
    if (!tools.includes(toolName)) return; // leash: never call a tool the cell didn't declare.
    void bridge.call(toolName, (a.context ?? {}) as Record<string, unknown>);
  };

  if (parsed === "draft") {
    // Un-authored draft: guide the author to generate it, not an error. Matches "blank timeseries".
    return (
      <div className="flex h-full flex-col" data-view="genui">
        <WidgetHeader label={label ?? ""} />
        <WidgetMessage tone="muted">
          AI widget — open the editor's “AI widget” tab and describe it to generate.
        </WidgetMessage>
      </div>
    );
  }
  if (parsed === "invalid" || !ir) {
    return <WidgetMessage tone="denied">invalid genui widget (malformed IR)</WidgetMessage>;
  }

  // Every target denied → the standard denied panel state, exactly like any other view (no genui-
  // specific deny UX). Partial denial renders the surface; a denied binding resolves to no value.
  const denied = targets.length > 0 && targets.every((t) => byRef[t.refId]?.denied === true) &&
    Object.keys(byRef).length >= targets.length;

  return (
    <div className="flex h-full flex-col" aria-label={`genui ${label ?? ""}`} data-view="genui">
      <WidgetHeader label={label ?? ""} />
      {targets.map((t) => (
        <TargetProbe
          key={t.refId}
          parent={cell}
          target={t}
          scope={scope}
          refreshKey={refreshKey}
          onData={onData}
        />
      ))}
      {denied ? (
        <WidgetMessage tone="denied">no access to this source</WidgetMessage>
      ) : (
        <div className="min-h-0 flex-1">
          <GenUiSurface spec={ir} data={model as unknown as Record<string, unknown>} catalog={nubeCatalog} bridge={bridge} onAction={onAction} />
        </div>
      )}
    </div>
  );
}
