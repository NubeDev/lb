// The catalog section renderer — one section's header/hint + its honest state body (system-catalog
// scope, extracted from the rules panel's `DataExplorer` `Section` + `render` helpers). The renderer
// is kind-AGNOSTIC: it renders whatever `CatalogSectionSpec` is passed + the section's `SectionState`
// + a ready-body the host hands it (the rows). The row rendering is the explorer's concern (this
// component renders a section; `CatalogExplorer` renders rows for each kind).
//
// Self-themed via `--sp-*` tokens scoped to `.sp-root` (the @nube/panel discipline; no preflight, no
// global utilities — same as the combobox skin).

import type { ReactNode } from "react";

import type { SectionState } from "./types";
import type { CatalogSectionSpec } from "./catalog";

export interface CatalogSectionProps<T> {
  spec: CatalogSectionSpec;
  state: SectionState<T>;
  /** The ready-body renderer — receives the section's data and returns the row tree. The explorer
   *  composes this per kind (datasource rows / the schema table tree / channel rows / …). */
  children: (data: T) => ReactNode;
}

/** Render a section's header/hint + its honest state: loading skeleton, "Not permitted." deny, the
 *  teaching empty (when `ready` returns null/[]), or the ready body. */
export function CatalogSection<T>({ spec, state, children }: CatalogSectionProps<T>) {
  return (
    <section className="sp-catalog-section" aria-label={`section ${spec.label}`}>
      <header className="sp-catalog-section-head">
        <h3 className="sp-catalog-section-title">{spec.label}</h3>
        <p className="sp-catalog-section-hint">{spec.hint}</p>
      </header>
      {renderState(state, children)}
    </section>
  );
}

/** Render the section's body from its state — loading skeleton, deny, or the ready body (which may
 *  itself be a teaching empty if `children` returns null). */
function renderState<T>(state: SectionState<T>, ready: (data: T) => ReactNode): ReactNode {
  if (state.status === "loading") {
    return <div aria-label="loading" className="sp-catalog-skeleton" />;
  }
  if (state.status === "denied") {
    return (
      <p aria-label="denied" className="sp-catalog-denied">
        Not permitted.
      </p>
    );
  }
  return ready(state.data);
}

/** A teaching empty state — used by per-kind row renderers when the section is ready but holds zero
 *  rows (e.g. "No external datasources registered."). */
export function CatalogEmpty({ children }: { children: ReactNode }) {
  return <p className="sp-catalog-empty">{children}</p>;
}
