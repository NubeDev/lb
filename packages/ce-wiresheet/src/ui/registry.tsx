// Lean client-side renderer for the declarative UI DSL (src/lib/ui/types.ts).
// A registry maps a widget `type` → React component; a walker renders a node and
// recurses. Unknown types degrade to a visible placeholder (never throw). This
// reuses SDUI's registry *idea* but binds straight to the live value store —
// no server resolve. See ../../SDUI_UNIFIED_DESIGN.md §10.

import type { ComponentType } from "react";
import type { Widget } from "../lib/ui/types";
import type { FlexValue } from "../lib/engine-types";

/** Context threaded to every widget while rendering a view. */
export interface RenderCtx {
  /** the component this view is bound to (follow/sync selection), if any */
  componentUid?: number;
  /** invoke a component action (`POST /call/nodes/uid/{uid}`) → its `returns` */
  callAction?: (
    componentUid: number,
    name: string,
    params?: Record<string, FlexValue>,
  ) => Promise<Record<string, FlexValue>>;
  /**
   * Subscribe a set of property uids to the live WS value stream while the widget
   * is mounted, so a panel bound to an off-canvas (cross-folder) component still
   * streams. Returns a cleanup that drops the subscription. Replaces the active
   * widget's prop set on each call (one drawer widget is shown at a time).
   */
  subscribeProps?: (propUids: number[]) => () => void;
  /** Write a value to a component property (maps to the host's set-default path).
   *  Used by widgets that change a config/input prop, e.g. a jsLogic's scriptId. */
  setValue?: (componentUid: number, property: string, value: FlexValue) => void;
  /** Whether this widget is the currently-visible one (e.g. the active editor
   *  tab). Mounted-but-inactive widgets should skip the value subscription, since
   *  the host's prop-subscription holds a single active set. Defaults to active. */
  active?: boolean;
  /** Navigate the canvas to a component (drill into its folder + center/select).
   *  Powers "locate" buttons that jump from a drawer view to the node. */
  locate?: (componentUid: number) => void;
  /** One-shot request to open/focus a specific component's tab inside a
   *  per-component editor (the forward of `locate`: canvas right-click "Open UX"
   *  → this component's panel). `focusNonce` changes each request so re-opening
   *  the same uid re-triggers. */
  focusUid?: number;
  focusNonce?: number;
}

export interface WidgetProps {
  node: Widget;
  ctx: RenderCtx;
}

const REGISTRY: Record<string, ComponentType<WidgetProps>> = {};

export function registerWidget(type: string, comp: ComponentType<WidgetProps>): void {
  REGISTRY[type] = comp;
}

export function lookupWidget(type: string): ComponentType<WidgetProps> | undefined {
  return REGISTRY[type];
}

export function listWidgets(): string[] {
  return Object.keys(REGISTRY);
}

/** Render one widget node by looking up its `type` in the registry. */
export function RenderWidget({ node, ctx }: WidgetProps) {
  const C = lookupWidget(node.type);
  if (!C) {
    return (
      <div
        style={{
          border: "1px dashed hsl(var(--crit))",
          color: "hsl(var(--crit))",
          padding: "4px 8px",
          fontSize: 11,
          borderRadius: 3,
        }}
      >
        unknown widget: {node.type}
      </div>
    );
  }
  return <C node={node} ctx={ctx} />;
}
