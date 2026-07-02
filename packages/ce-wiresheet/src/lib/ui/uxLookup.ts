// Right-click "Open UX" lookup: given a component's full type, find the
// extension UI that provides a per-component editor for it. Availability and
// behaviour are entirely descriptor-driven — these come from the local stub
// today and from the engine's GET /ui/list later, so the right-click reads the
// same source either way (see COMPONENT_UX_DESIGN.md).

import type { ExtensionUi } from "./types";

// Widgets that edit ONE component (so "Open UX" can focus the clicked node).
// Type-agnostic widgets (table/tree/collection) aren't per-component editors.
const PER_COMPONENT_WIDGETS = new Set(["tabbedEditor", "jsComponents"]);

const lastSeg = (t: string) => t.toLowerCase().split(/[:/.\\]+/).filter(Boolean).pop() ?? t.toLowerCase();

function typeMatches(a: string | undefined, b: string | undefined): boolean {
  if (!a || !b) return false;
  const x = a.toLowerCase(), y = b.toLowerCase();
  return x === y || lastSeg(x) === lastSeg(y);
}

export interface UxTarget {
  extId: string;
  uiId: string;
}

/** Find the extension UI providing a per-component editor for `fullType`, plus
 *  the widget type that hosts it. Returns the first match, or null when no UI
 *  targets the type (→ no "Open UX" item). */
export function findComponentUx(extensions: ExtensionUi[], fullType: string | undefined): UxTarget | null {
  if (!fullType) return null;
  for (const ext of extensions) {
    for (const ui of ext.uis) {
      if (ui.view.type !== "layout") continue;
      for (const w of ui.view.children) {
        if (!PER_COMPONENT_WIDGETS.has(w.type)) continue;
        const widgetType = typeof w.fullType === "string" ? w.fullType : undefined;
        if (typeMatches(widgetType, fullType)) return { extId: ext.id, uiId: ui.id };
      }
    }
  }
  return null;
}
