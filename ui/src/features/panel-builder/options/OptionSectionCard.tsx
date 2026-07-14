// The reusable per-option ROW (panel-wizard scope, redesigned per the scope's resolved decision #3:
// ONE `OptionFocusPreview`, never per-option renderers). The row renders the option's `<Control>` and,
// for a DEAD option (per `optionLiveness`), the honest "no visible effect — renderer pending" note.
// It renders NO chart of its own — the single pinned preview is the only render surface; the row just
// REPORTS focus (hover / focus-within) upward via `onFocus` so the host can point the pinned
// `OptionFocusPreview` at the option being edited.
//
// Why this is one reusable row and not wizard-only: the Field-tab port-back (Phase 2) composes the SAME
// row. Both surfaces read/write through `writeOption` — the no-drift guarantee is structural. The row
// owns NO state of its own; it is a pure composition of the registry's `Control` and `optionLiveness`.
//
// One responsibility: render one option row (control + dead note + focus reporting).

import type { View } from "@/lib/dashboard";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import type { OptionDef } from "./types";
import { readOption, writeOption } from "./binding";
import { optionLiveness } from "./optionLiveness";
import { Control } from "./Control";

interface Props {
  /** The option this row renders. */
  def: OptionDef;
  /** The current view — selects the row in `optionLiveness` (live vs dead for this option). */
  view: View;
  /** The working `EditorState` — the source of truth the row reads/writes (no row-local state). */
  state: EditorState;
  /** Apply a state patch (the host's `setState`-equivalent). */
  patch: (next: Partial<EditorState>) => void;
  /** Report that this option is being edited (hover or focus) — the host points the ONE pinned
   *  `OptionFocusPreview` at it. Absent ⇒ the row is inert (no focus reporting). */
  onFocus?: (optionId: string) => void;
  /** Is this the option the host's preview is currently focusing? Drives the row highlight only. */
  focused?: boolean;
}

/** The rich controls that render full-width UNDER their label (mirrors `OptionGroups`'s BLOCK_CONTROLS). */
const BLOCK_CONTROLS = new Set(["thresholds", "mappings", "color-scheme", "data-links"]);

export function OptionSectionCard({ def, view, state, patch, onFocus, focused = false }: Props) {
  const value = readOption(state, def);
  const set = (v: unknown) => patch(writeOption(state, def, v));
  const live = optionLiveness(view, def.id);
  const block = BLOCK_CONTROLS.has(def.control.kind);
  const report = onFocus ? () => onFocus(def.id) : undefined;

  return (
    <section
      className={`grid gap-1.5 px-3 py-2.5 text-xs transition-colors ${
        focused ? "bg-accent/10" : "hover:bg-muted/10"
      }`}
      aria-label={`option section ${def.id}`}
      data-option-id={def.id}
      data-live={live ? "true" : "false"}
      onPointerEnter={report}
      onFocusCapture={report}
    >
      {block ? (
        <>
          <div className="font-medium text-fg">{def.label}</div>
          <Control control={def.control} label={def.label} value={value} onChange={set} />
        </>
      ) : (
        <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1.1fr)] items-center gap-3">
          <div className="font-medium text-fg">{def.label}</div>
          <Control control={def.control} label={def.label} value={value} onChange={set} />
        </div>
      )}
      {!live && (
        <p className="text-muted italic" role="note">
          no visible effect — renderer pending
        </p>
      )}
    </section>
  );
}
