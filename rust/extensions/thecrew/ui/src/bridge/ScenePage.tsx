// The full graphics PAGE shell (the `[ui]` mount): the lifted playground `<App/>` (palette + canvas
// + rail + toolbar, all unchanged) wrapped with the two things the extension adds — a persistence
// bar (scene picker + save, over `assets.*`) and the bridge-backed ValueSource (bindings under the
// viewer's grant). The App stays prop-less and reads the store singleton; this shell drives the
// store's `loadDoc` from a fetched scene and re-derives the ValueSource from the doc's channels.

import { useCallback, useEffect, useMemo, useState } from "react";
import { App } from "../App";
import { ValueSourceContext } from "../data/use-values";
import { useSceneStore } from "../state/scene-store";
import type { Bridge } from "./contract";
import { createBridgeSource, collectChannels } from "./bridge-source";
import {
  listScenes,
  loadScene,
  saveScene,
  SceneConflictError,
  type LoadedScene,
  type SceneSummary,
} from "./scene-io";

type SaveState = { kind: "idle" | "saving" | "saved" } | { kind: "denied" | "conflict"; msg: string };

export function ScenePage({ bridge }: { bridge: Bridge }) {
  const doc = useSceneStore((s) => s.doc);
  const loadDoc = useSceneStore((s) => s.loadDoc);

  const [scenes, setScenes] = useState<SceneSummary[]>([]);
  const [loaded, setLoaded] = useState<LoadedScene | null>(null);
  const [title, setTitle] = useState("Untitled scene");
  const [save, setSave] = useState<SaveState>({ kind: "idle" });

  // One ValueSource per bound-channel set: rebuild only when the channels actually change, so a
  // pure move/rename doesn't tear down live subscriptions.
  const channelKey = useMemo(() => collectChannels(doc).sort().join("|"), [doc]);
  const source = useMemo(
    () => createBridgeSource(bridge, channelKey ? channelKey.split("|") : []),
    [bridge, channelKey],
  );

  // The scene picker: list on mount (denied → empty list, the toolbar still works on the demo).
  useEffect(() => {
    listScenes(bridge)
      .then(setScenes)
      .catch(() => setScenes([]));
  }, [bridge]);

  const openScene = useCallback(
    async (id: string) => {
      try {
        const s = await loadScene(bridge, id);
        setLoaded(s);
        setTitle(s.title);
        loadDoc(s.doc);
        setSave({ kind: "idle" });
      } catch {
        setSave({ kind: "denied", msg: "could not load scene" });
      }
    },
    [bridge, loadDoc],
  );

  const doSave = useCallback(async () => {
    setSave({ kind: "saving" });
    const id = loaded?.id ?? (title || "untitled");
    try {
      const next = await saveScene(bridge, { id, title, doc, loaded: loaded ?? undefined });
      setLoaded(next);
      setSave({ kind: "saved" });
      listScenes(bridge).then(setScenes).catch(() => {});
    } catch (err) {
      if (err instanceof SceneConflictError) {
        // Honest last-writer-wins interim: never a silent clobber (thecrew-extension-scope §Risks).
        setSave({ kind: "conflict", msg: "scene changed underneath you — reload?" });
      } else {
        // A denied save surfaces the deny honestly in the bar (deny path, testing plan).
        setSave({ kind: "denied", msg: "save denied" });
      }
    }
  }, [bridge, doc, loaded, title]);

  return (
    <ValueSourceContext.Provider value={source}>
      <div className="flex h-screen flex-col">
        <div
          data-testid="scene-persistence-bar"
          className="flex h-9 shrink-0 items-center gap-2 border-b border-[var(--tc-hairline)] bg-[var(--tc-panel)] px-3 text-xs text-slate-300 backdrop-blur-md"
        >
          <select
            data-testid="scene-picker"
            className="rounded bg-transparent px-1 py-0.5 outline-none"
            value={loaded?.id ?? ""}
            onChange={(e) => e.target.value && openScene(e.target.value)}
          >
            <option value="">— open scene —</option>
            {scenes.map((s) => (
              <option key={s.id} value={s.id}>
                {s.title}
              </option>
            ))}
          </select>
          <input
            data-testid="scene-title"
            className="w-40 rounded bg-transparent px-1 py-0.5 outline-none"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
          />
          <button
            type="button"
            data-testid="scene-save"
            onClick={doSave}
            className="rounded px-2 py-0.5 text-[var(--tc-accent)] hover:bg-white/5"
          >
            Save
          </button>
          {save.kind === "saved" && <span data-testid="scene-status">saved</span>}
          {save.kind === "saving" && <span data-testid="scene-status">saving…</span>}
          {(save.kind === "denied" || save.kind === "conflict") && (
            <span data-testid="scene-status" className="text-amber-400">
              {save.msg}
              {save.kind === "conflict" && loaded && (
                <button
                  type="button"
                  data-testid="scene-reload"
                  onClick={() => openScene(loaded.id)}
                  className="ml-2 underline"
                >
                  reload
                </button>
              )}
            </span>
          )}
        </div>
        <div className="min-h-0 flex-1">
          <App />
        </div>
      </div>
    </ValueSourceContext.Provider>
  );
}
