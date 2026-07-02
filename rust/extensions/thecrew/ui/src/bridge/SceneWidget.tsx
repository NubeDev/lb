// The read-only scene CELL (the `[[widget]]` mount): a dashboard cell that renders a scene and its
// live values but can NEVER save one (its manifest scope omits `assets.put_doc`/`list_docs`). Just
// `<SceneCanvas>` (the lifted renderer) over the bridge ValueSource — no palette, rail, toolbar, or
// persistence bar. A viewer missing `series.read`/`series.watch` sees the shapes' no-access state
// (null values), the scene still renders (deny path, testing plan).

import { useEffect, useMemo, useState } from "react";
import { SceneCanvas } from "../canvas/SceneCanvas";
import { ValueSourceContext } from "../data/use-values";
import { useSceneStore } from "../state/scene-store";
import type { WidgetBridge } from "./contract";
import { createBridgeSource, collectChannels } from "./bridge-source";
import { loadScene } from "./scene-io";

/** Render scene `sceneId` read-only. Loads the doc into the shared store, then renders the canvas
 *  with a bridge ValueSource built from the doc's bound channels. */
export function SceneWidget({ bridge, sceneId }: { bridge: WidgetBridge; sceneId: string }) {
  const doc = useSceneStore((s) => s.doc);
  const loadDoc = useSceneStore((s) => s.loadDoc);
  const [ready, setReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    loadScene(bridge, sceneId)
      .then((s) => {
        if (cancelled) return;
        loadDoc(s.doc);
        setReady(true);
      })
      .catch(() => {
        // A denied/absent scene renders an honest empty state, never a crash.
        if (!cancelled) setError("scene unavailable");
      });
    return () => {
      cancelled = true;
    };
  }, [bridge, sceneId, loadDoc]);

  const channelKey = useMemo(() => collectChannels(doc).sort().join("|"), [doc]);
  const source = useMemo(
    () => createBridgeSource(bridge, channelKey ? channelKey.split("|") : []),
    [bridge, channelKey],
  );

  if (error) {
    return (
      <div
        data-testid="scene-widget-empty"
        className="flex h-full w-full items-center justify-center bg-[var(--tc-canvas)] text-xs text-[var(--tc-text-muted)]"
      >
        {error}
      </div>
    );
  }
  if (!ready) {
    return (
      <div
        data-testid="scene-widget-loading"
        className="flex h-full w-full items-center justify-center bg-[var(--tc-canvas)] text-xs text-[var(--tc-text-muted)]"
      >
        loading scene…
      </div>
    );
  }
  return (
    <ValueSourceContext.Provider value={source}>
      <div data-testid="scene-widget" className="h-full w-full">
        {/* `fit`: auto-frame the scene into the cell + lock pan/zoom (parent-scope "fit per cell" risk —
            the editor's fixed ±350/zoom-1.6 crop rendered blank in a small grid tile). */}
        <SceneCanvas fit />
      </div>
    </ValueSourceContext.Provider>
  );
}
