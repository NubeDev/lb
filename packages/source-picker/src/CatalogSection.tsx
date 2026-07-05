// The catalog section renderer — one COLLAPSIBLE section header + its honest state body (system-
// catalog scope, extracted from the rules panel's `DataExplorer`). The renderer is kind-AGNOSTIC:
// it renders whatever `CatalogSectionSpec` is passed + the section's `SectionState` + a ready-body
// the host hands it (the rows). The row rendering is the explorer's concern (this component renders
// a section; `CatalogExplorer` renders rows for each kind).
//
// LAZY LOAD CONTRACT: the section starts COLLAPSED when its state is `idle` (the loader hasn't fired
// yet). The first expand calls `onOpen`, which the host wires to `loadSection(kind)` — that's the
// moment the section's `*.list` API call goes out. Once the state is `loading`/`ready`/`denied`, the
// section stays open (the data is cached). Subsequent collapse/re-expand doesn't refire the loader.
//
// The COLLAPSIBLE primitive is shadcn's file-tree pattern (sidebar-11): Radix Collapsible + a
// `ChevronRight` that rotates 90° on open. Self-themed via `--sp-*` tokens scoped to `.sp-root`.

import { useState, type ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import * as CollapsiblePrimitive from "@radix-ui/react-collapsible";

import type { SectionState } from "./types";
import type { CatalogSectionSpec } from "./catalog";

export interface CatalogSectionProps<T> {
  spec: CatalogSectionSpec;
  state: SectionState<T>;
  /** Fired the first time the user expands a section whose state is still `idle` — the host's cue to
   *  trigger this section's loader. The collapsible handles its own open/close thereafter; this is
   *  the lazy-load trigger, not an open/close controller. Optional (a host that pre-seeds `ready`
   *  state never triggers it). */
  onOpen?: () => void;
  /** Force the section open on first mount (default: open iff `state` is past `idle`). Tests + hosts
   *  that pre-seed `ready` data pass `defaultOpen` so rows render without a click. */
  defaultOpen?: boolean;
  /** The ready-body renderer — receives the section's data and returns the row tree. The explorer
   *  composes this per kind (datasource rows / the schema table tree / channel rows / …). */
  children: (data: T) => ReactNode;
}

/** A collapsible section: a clickable header (chevron + title + hint) + the body. The header toggles
 *  open/close; the first open of an `idle` section fires `onOpen` so the host can lazy-load it. */
export function CatalogSection<T>({ spec, state, onOpen, defaultOpen, children }: CatalogSectionProps<T>) {
  // Default open iff the host pre-seeded non-idle state (loading/ready/denied) — so render tests that
  // pass `ready` data see rows immediately, and a section mid-load on remount stays open.
  const [open, setOpen] = useState(defaultOpen ?? state.status !== "idle");
  const wasIdle = state.status === "idle";

  const onOpenChange = (next: boolean) => {
    setOpen(next);
    // First expansion of an idle section ⇒ fire the loader.
    if (next && wasIdle && onOpen) onOpen();
  };

  return (
    <CollapsiblePrimitive.Root
      className="sp-catalog-section"
      aria-label={`section ${spec.label}`}
      open={open}
      onOpenChange={onOpenChange}
    >
      <CollapsiblePrimitive.Trigger
        className="sp-catalog-section-head"
        aria-label={`toggle section ${spec.label}`}
      >
        <ChevronRight className="sp-catalog-section-chevron" />
        <h3 className="sp-catalog-section-title">{spec.label}</h3>
        <p className="sp-catalog-section-hint">{spec.hint}</p>
      </CollapsiblePrimitive.Trigger>
      <CollapsiblePrimitive.Content className="sp-catalog-section-content">
        {renderState(state, children)}
      </CollapsiblePrimitive.Content>
    </CollapsiblePrimitive.Root>
  );
}

/** Render the section's body from its state — idle hint, loading skeleton, deny, or the ready body
 *  (which may itself be a teaching empty if `children` returns null). */
function renderState<T>(state: SectionState<T>, ready: (data: T) => ReactNode): ReactNode {
  if (state.status === "idle") {
    return <p className="sp-catalog-idle">Expand to load.</p>;
  }
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
