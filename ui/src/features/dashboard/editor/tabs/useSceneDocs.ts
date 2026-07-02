// Load the workspace's scene docs for the ext-widget options field (thecrew finding 8: let the builder
// set a Scene tile's `sceneId` instead of hand-seeding the cell). It reads the SHIPPED, ws-walled
// `assets.list_docs` verb and filters to the `scene:` id-prefix convention the thecrew scene-io uses
// (list_docs returns no tags, so the prefix IS the discovery key — same resolution the extension made).
// A denied/empty list is an HONEST empty roster (the field shows "no scenes"), never a fabricated one.
//
// One hook per file (FILE-LAYOUT). Generic: any ext widget whose config is a doc id can reuse a
// prefix; thecrew's Scene tile uses `scene:`.

import { useEffect, useState } from "react";

import { listDocs } from "@/lib/assets/assets.api";

/** One selectable scene doc — `{id, title}` (all `list_docs` returns; no tags, no content). */
export interface SceneDocOption {
  id: string;
  title: string;
}

/** Load the `scene:`-prefixed docs in `ws`. Re-loads on a workspace switch; denied → empty (honest). */
export function useSceneDocs(ws: string): { scenes: SceneDocOption[]; loading: boolean } {
  const [scenes, setScenes] = useState<SceneDocOption[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    listDocs(ws)
      .then((docs) => {
        if (cancelled) return;
        setScenes(
          docs
            .filter((d) => d.id.startsWith("scene:"))
            .map((d) => ({ id: d.id, title: d.title || d.id })),
        );
        setLoading(false);
      })
      .catch(() => {
        if (cancelled) return;
        setScenes([]); // denied/failed → an honest empty roster, never invented docs
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [ws]);

  return { scenes, loading };
}
