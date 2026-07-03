// Save a built draft as a LIBRARY PANEL (panel-kit) — the workbench's primary output. Wraps the
// shipped `panel.save` asset verbs (`@/lib/panel`): strip the draft's geometry/ref fields to a
// `PanelSpec` and persist under a caller-chosen permanent slug. The host re-checks
// `mcp:panel.save:call` + the workspace wall regardless of any UI gate.

import type { Cell } from "@/lib/dashboard";
import { cellToSpec, savePanel, type Panel } from "@/lib/panel";

/** Derive a stable id slug from a title (the library-panels default-slug rule). */
export function slugify(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "panel";
}

/** Persist `draft` as the library panel `id` (slugified; permanent at creation). Returns the saved
 *  panel — reusable on any dashboard (ref cell) and standalone at `/t/$ws/panel/{id}`. */
export function saveDraftAsPanel(draft: Cell, id: string, title?: string): Promise<Panel> {
  const t = title?.trim() || draft.title?.trim() || "Panel";
  return savePanel(slugify(id), t, cellToSpec(draft));
}
