// Ambient "freeze" for the panel-data fetch (data-studio-ux scope, edit-without-requery). Freeze must
// reach the ACTUAL rendered preview, whose renderers each call `usePanelData` deep inside `WidgetView` —
// threading a `frozen` prop through every view would touch a dozen files and re-couple the render path.
// Instead freeze is AMBIENT, like the ws context: the editor wraps its preview subtree in a provider and
// `useVizQuery` reads it. Outside the editor (the live dashboard) there is no provider → never frozen, so
// the render path is unchanged. One responsibility: carry the freeze flag to the data hook.

import { createContext, useContext } from "react";

/** Frozen = the datasource is not re-hit; the last cached raw frames are reshaped instead. Default false
 *  (no provider → the dashboard render path fetches normally). */
const FreezeContext = createContext(false);

export const FreezeProvider = FreezeContext.Provider;

/** Read the ambient freeze flag. An explicit `useVizQuery({frozen})` opt takes precedence over this. */
export function useFreeze(): boolean {
  return useContext(FreezeContext);
}
