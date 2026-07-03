// The workbench layout's load/persist seam (data-studio scope v2, "Layout persistence") — one hook
// owning the FlexLayout `Model` lifecycle: load the caller's saved layout from the MEMBER-OWNED
// SurrealDB record (`layout.get`, `ui_layout:[ws, user, surface]` — rule 4: never localStorage), fall
// back to the default workbench, and debounce-save every model change back through `layout.set`. The
// model JSON carries each tab's draft-cell `config`, so what persists is the whole debugging setup.

import { useEffect, useRef, useState } from "react";
import { Model, type IJsonModel } from "flexlayout-react";

import { getLayout, setLayout } from "@/lib/layout";
import { CAP, hasCap } from "@/lib/session";

import { DATA_STUDIO_SURFACE, defaultWorkbenchModel } from "./workbenchModel";

const SAVE_DEBOUNCE_MS = 800;

export interface WorkbenchLayout {
  /** The live FlexLayout model (null while the saved layout loads — render nothing yet). */
  model: Model | null;
  /** Feed to `<Layout onModelChange>` — schedules the debounced persist. */
  onModelChange: () => void;
  /** Reset to the default workbench (also persisted). */
  reset: () => void;
}

/** Try a saved model JSON; a corrupt/foreign shape falls back to the default (never a blank page). */
function modelFrom(saved: unknown): Model {
  if (saved && typeof saved === "object") {
    try {
      return Model.fromJson(saved as IJsonModel);
    } catch {
      // fall through — a layout from an older shape renders the default rather than crashing.
    }
  }
  return Model.fromJson(defaultWorkbenchModel());
}

export function useWorkbenchLayout(ws: string, caps: string[] | undefined): WorkbenchLayout {
  const [model, setModel] = useState<Model | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const canSave = hasCap(caps, CAP.layoutSet);

  // Load the caller's saved arrangement once per workspace visit.
  useEffect(() => {
    let live = true;
    setModel(null);
    getLayout(DATA_STUDIO_SURFACE)
      .then((l) => live && setModel(modelFrom(l.model)))
      .catch(() => live && setModel(Model.fromJson(defaultWorkbenchModel())));
    return () => {
      live = false;
      if (timer.current) clearTimeout(timer.current);
    };
  }, [ws]);

  const persist = (m: Model) => {
    if (!canSave) return; // no grant → session-local only; the host would deny anyway.
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void setLayout(DATA_STUDIO_SURFACE, m.toJson()).catch(() => {
        // A failed save keeps the in-memory layout; the next change retries.
      });
    }, SAVE_DEBOUNCE_MS);
  };

  return {
    model,
    onModelChange: () => {
      if (model) persist(model);
    },
    reset: () => {
      const fresh = Model.fromJson(defaultWorkbenchModel());
      setModel(fresh);
      persist(fresh);
    },
  };
}
