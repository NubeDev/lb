// The library-panel affordances inside the panel editor (library-panels scope, editor step d):
// Save-as-library, the "used on N dashboards" banner, Save-to-library, and Unlink — the whole
// link/unlink lifecycle in one bar. It drives the REAL `panel.*` verbs (no local state): the host is
// the authority, the gateway re-checks each cap. One responsibility: the editor's library controls.
//
//  - An INLINE cell (no `panelRef`): "Save as library panel" → `panel.save` (spec extracted) → the
//    cell becomes a REF (the parent splices the ref in). Editing it later edits the shared record.
//  - A REF cell (hydrated `panelRef` set): a banner "Library panel — used on N dashboards" (from
//    `panel.usage`) + "Save to library" (edits the SHARED record via `panel.save`) + "Unlink" (copies
//    the spec back inline — drift becomes explicit and the caller's own).

import { useEffect, useState } from "react";
import { Library, Unlink } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Cell } from "@/lib/dashboard";
import { cellToSpec, refCell, unlinkCell, savePanel, panelUsage } from "@/lib/panel";
import { CAP, getSession, hasCap } from "@/lib/session";

interface Props {
  /** The current draft cell (what save would persist) — its spec is the panel payload. */
  draft: Cell;
  /** Persist the resulting cell into the dashboard (ref cell after save-as-library / inline after
   *  unlink) — the same `onSave` the editor's Save button uses; the parent saves the dashboard. */
  onSave: (cell: Cell) => void;
}

/** Derive a stable slug from a title (lowercase, hyphenated) — the creation-time default (editable via
 *  the prompt; the slug is forever after, the record renames its title only). */
function slugify(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "panel";
}

export function LibraryPanelBar({ draft, onSave }: Props) {
  const caps = getSession()?.caps ?? [];
  const isRef = !!draft.panelRef;
  const refId = draft.panelRef?.replace(/^panel:/, "") ?? "";
  const [usage, setUsage] = useState<number | null>(null);
  const [error, setError] = useState<string | undefined>();

  // A ref cell reads its usage for the "used on N dashboards" banner (best-effort; a viewer lacking
  // `panel.usage` simply sees no count, never an error surface).
  useEffect(() => {
    if (!isRef || !hasCap(caps, CAP.panelUsage)) return;
    let live = true;
    panelUsage(refId)
      .then((rows) => live && setUsage(rows.length))
      .catch(() => live && setUsage(null));
    return () => {
      live = false;
    };
  }, [isRef, refId, caps]);

  if (!hasCap(caps, CAP.panelSave)) return null;

  const saveAsLibrary = async () => {
    const title = draft.title?.trim() || "Panel";
    const id = window.prompt("Library panel id (permanent slug):", slugify(title));
    if (!id) return;
    try {
      const saved = await savePanel(slugify(id), title, cellToSpec(draft));
      onSave(refCell(draft, saved.id)); // the cell becomes a ref to the shared record
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const saveToLibrary = async () => {
    try {
      await savePanel(refId, draft.title?.trim() || "Panel", cellToSpec(draft));
      onSave(refCell(draft, refId)); // keep the cell a ref; the shared record now carries the edit
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="flex flex-wrap items-center gap-2 rounded-md border border-border bg-panel px-3 py-2 text-xs">
      {isRef ? (
        <>
          <Library size={13} className="text-accent" />
          <span className="font-medium" data-testid="library-banner">
            Library panel{usage !== null ? ` — used on ${usage} dashboard${usage === 1 ? "" : "s"}` : ""}
          </span>
          <div className="ml-auto flex gap-2">
            <Button size="sm" variant="outline" onClick={() => void saveToLibrary()}>
              Save to library
            </Button>
            <Button
              size="sm"
              variant="ghost"
              aria-label="unlink library panel"
              onClick={() => onSave(unlinkCell(draft))}
            >
              <Unlink size={12} /> Unlink
            </Button>
          </div>
        </>
      ) : (
        <>
          <span className="text-muted">Reuse this panel across dashboards.</span>
          <Button
            className="ml-auto"
            size="sm"
            variant="outline"
            aria-label="save as library panel"
            onClick={() => void saveAsLibrary()}
          >
            <Library size={12} /> Save as library panel
          </Button>
        </>
      )}
      {error && (
        <span className="w-full text-destructive" role="alert">
          {error}
        </span>
      )}
    </div>
  );
}
