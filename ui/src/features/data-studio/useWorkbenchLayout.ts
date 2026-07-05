// The workbench layout's load/persist seam (data-studio-10x scope, phase 1) — one hook owning the
// Dockview lifecycle: load the caller's saved layout from the MEMBER-OWNED SurrealDB record
// (`layout.get`, `ui_layout:[ws, user, surface]` — rule 4: never localStorage), restore it into the
// dock on ready, and debounce-save every layout change back through `layout.set` as the VERSIONED
// `{engine:"dockview", model}` record. A legacy (flexlayout-era) blob falls back to the default
// workbench and raises the one-time "layout was reset" notice. The serialized dock carries each
// pane's draft-cell `params`, so what persists is the whole debugging setup.

import { useEffect, useRef, useState } from "react";
import type { DockviewApi, DockviewReadyEvent } from "dockview-react";

/** Dockview's event-subscription handle (the lib doesn't re-export its `IDisposable`). */
interface Disposable {
  dispose(): void;
}

import { getLayout, setLayout } from "@/lib/layout";
import { CAP, hasCap } from "@/lib/session";

import { DATA_STUDIO_SURFACE, loadWorkbench, workbenchRecord, type LoadedWorkbench } from "./workbenchModel";

const SAVE_DEBOUNCE_MS = 800;

export interface WorkbenchLayout {
  /** True once the saved layout loaded — mount the dock only then (onReady restores it). */
  ready: boolean;
  /** The live Dockview api (null until the dock mounts) — `addPanel` etc. */
  api: DockviewApi | null;
  /** Feed to `<DockviewReact onReady>` — restores the saved model + wires the persist. */
  onReady: (event: DockviewReadyEvent) => void;
  /** Schedule a persist for changes Dockview's layout event can't see (pane param updates). */
  touch: () => void;
  /** Reset to the default (empty) workbench (also persisted). */
  reset: () => void;
  /** One-time notice: a saved layout existed but was from the old engine and was reset. */
  resetNotice: boolean;
  dismissResetNotice: () => void;
}

export function useWorkbenchLayout(ws: string, caps: string[] | undefined): WorkbenchLayout {
  const [api, setApi] = useState<DockviewApi | null>(null);
  const [loaded, setLoaded] = useState<LoadedWorkbench | null>(null);
  const [resetNotice, setResetNotice] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const listener = useRef<Disposable | null>(null);
  const canSave = hasCap(caps, CAP.layoutSet);

  // Load the caller's saved arrangement once per workspace visit.
  useEffect(() => {
    let live = true;
    setLoaded(null);
    setApi(null);
    getLayout(DATA_STUDIO_SURFACE)
      .then((l) => {
        if (!live) return;
        const parsed = loadWorkbench(l.model);
        setLoaded(parsed);
        setResetNotice(parsed.wasReset);
      })
      .catch(() => live && setLoaded({ model: null, wasReset: false }));
    return () => {
      live = false;
      if (timer.current) clearTimeout(timer.current);
      listener.current?.dispose();
      listener.current = null;
    };
  }, [ws]);

  const persist = (a: DockviewApi) => {
    if (!canSave) return; // no grant → session-local only; the host would deny anyway.
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void setLayout(DATA_STUDIO_SURFACE, workbenchRecord(a.toJSON())).catch(() => {
        // A failed save keeps the in-memory layout; the next change retries.
      });
    }, SAVE_DEBOUNCE_MS);
  };

  const onReady = (event: DockviewReadyEvent) => {
    const a = event.api;
    if (loaded?.model) {
      try {
        a.fromJSON(loaded.model);
      } catch {
        // A dockview-tagged record that still fails to restore → default + the notice.
        a.clear();
        setResetNotice(true);
      }
    }
    listener.current?.dispose();
    // Adds/moves/closes/resizes all funnel through onDidLayoutChange; param edits go via touch().
    listener.current = a.onDidLayoutChange(() => persist(a));
    setApi(a);
  };

  return {
    ready: loaded !== null,
    api,
    onReady,
    touch: () => {
      if (api) persist(api);
    },
    reset: () => {
      if (!api) return;
      api.clear();
      persist(api);
    },
    resetNotice,
    dismissResetNotice: () => setResetNotice(false),
  };
}
