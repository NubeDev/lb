// `<GenUiSurface>` — the render-stratum entry every viewer mounts. Walks the flat IR component map from
// the surface root, resolves each component's bindings against the data model, dispatches to the
// catalog `render` fn, and threads a leashed `emit` for controls. NO parser, NO normalize here — the
// spec arrives already-typed (parse/normalize/validate ran ONCE at authoring accept). Missing/unknown
// components render an inert placeholder (defense-in-depth), never a throw — a viewer must never crash
// on a spec the author's accept let through.

import { Fragment, type ReactNode } from "react";
import type { Catalog } from "../catalog/defineCatalog";
import type { DataModel, IrSpec, UiAction } from "../ir/types";
import { resolveBindings } from "../ir/resolveBindings";
import { GenUiProvider, type GenUiBridge } from "./GenUiContext";

export interface GenUiSurfaceProps {
  spec: IrSpec;
  data?: DataModel;
  catalog: Catalog;
  /** The bridge the host injects for control actions (dashboard: the widget iframe bridge). */
  bridge?: GenUiBridge;
  /** Called for every control action BEFORE the bridge call — lets the host log/leash-check. The
   *  catalog entry's declared `tool` for the action (if any) is looked up by the host from the cell. */
  onAction?: (action: UiAction) => void;
}

/** Render one component id (and, recursively, its children) against the resolved data model. */
function renderNode(
  id: string,
  spec: IrSpec,
  data: DataModel,
  catalog: Catalog,
  emit: (componentId: string, name: string, context?: Record<string, unknown>) => void,
  seen: Set<string>,
): ReactNode {
  if (seen.has(id)) return null; // cycle guard — orphan-tolerant, never infinite-loop.
  const comp = spec.components[id];
  if (!comp) {
    return (
      <div key={id} className="gu-missing" role="status">
        missing component: {id}
      </div>
    );
  }
  const entry = catalog.resolve(comp.component);
  const nextSeen = new Set(seen).add(id);
  const children = (comp.children ?? []).map((cid) => (
    <Fragment key={cid}>{renderNode(cid, spec, data, catalog, emit, nextSeen)}</Fragment>
  ));
  if (!entry) {
    // Defense-in-depth: an unknown component that slipped past accept renders inert, labelled.
    return (
      <div key={id} className="gu-unknown" role="status" data-component={comp.component}>
        unknown component: {comp.component}
      </div>
    );
  }
  const props = resolveBindings(comp.props, data);
  return (
    <Fragment key={id}>
      {entry.render({ props, children, emit: (name, context) => emit(id, name, context) })}
    </Fragment>
  );
}

export function GenUiSurface({ spec, data, catalog, bridge, onAction }: GenUiSurfaceProps) {
  const model: DataModel = data ?? spec.dataModel ?? {};
  const emit = (componentId: string, name: string, context?: Record<string, unknown>) => {
    onAction?.({ surfaceId: spec.surface.surfaceId, componentId, name, tool: name, context });
  };
  const root = spec.surface.root;
  return (
    <GenUiProvider value={{ bridge }}>
      <div className="gu-root gu-surface">
        {root ? renderNode(root, spec, model, catalog, emit, new Set()) : (
          <div className="gu-empty" role="status">
            empty widget
          </div>
        )}
      </div>
    </GenUiProvider>
  );
}
