// Source → draft-cell mapping (panel-kit; extracted from the studio's v1 `useExploreDraft`). A picked
// `@nube/source-picker` selection folds into a fresh draft `Cell`: the `{tool,args}` becomes the single
// v3 target `A` — exactly the shape `usePanelData`/`viz.query`/the editor state machine read — so the
// draft is immediately explorable, buildable, and savable. No parallel model: a draft IS a `Cell`.

import type { Cell, View } from "@/lib/dashboard";
import type { SourceSelection } from "@nube/source-picker";

import { defaultCell } from "./defaultCell";

/** Fold a picked source selection into a fresh draft cell of `view` keyed `key`. An extension-widget
 *  selection carries its own `viewKey` (the tile owns its data) — placed directly, no source binding.
 *  `options` is the per-view default option block (injected; see `defaultCell`). */
export function draftFromSelection(
  sel: SourceSelection,
  view: View = "timeseries",
  key = "explore",
  options: Record<string, unknown> = {},
): Cell {
  if (sel.viewKey) {
    return {
      ...defaultCell(view, key, undefined, options),
      view: sel.viewKey as View,
      sources: [],
      source: undefined,
    };
  }
  const cell = defaultCell(view, key, undefined, options);
  const tool = sel.source?.tool ?? "";
  const args = sel.source?.args ?? {};
  return {
    ...cell,
    sources: [{ refId: "A", tool, args, datasource: { type: "surreal" } }],
    // Keep the v2 `source` mirror in sync so the Query tab shows the picked tool immediately.
    source: { tool, args },
  };
}
