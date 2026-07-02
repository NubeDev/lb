// The bridge-backed `SourceLoaders` for `@nube/source-picker` (source-picker-package-scope.md, thecrew
// consumer). This is thecrew's half of the injected seam: the SAME reusable picker the dashboard uses,
// but reaching the node through the extension's host BRIDGE (`bridge.call`) instead of the shell's
// `@/lib/*` clients — the whole point of the package being transport-agnostic.
//
// thecrew's manifest grants only `series.*` (+ `assets.*`), so we implement `listSeries` (over
// `series.list`) — enough for a scene shape to bind a prop to any workspace series through the picker.
// Extensions/flows/datasources loaders are intentionally OMITTED: the manifest doesn't grant those
// reads, and a scene binds to series channels. Omitted loaders → those picker groups are simply absent
// (honest, capability-scoped), exactly the package's contract. The bridge re-gates every call
// server-side under the viewer's grant.

import type { SourceLoaders } from "@nube/source-picker";
import type { Bridge } from "./contract";

/** A `series.list` reply envelope: `{ series: string[] }`. */
interface SeriesListReply {
  series?: string[];
}

/** Build the picker's loaders over the extension bridge. Only the reads thecrew's grant covers are
 *  wired; the rest stay undefined (their groups don't appear). A denied/failed `series.list` throws —
 *  the package hook catches it and shows an empty Series group (never a fabricated list, CLAUDE §9). */
export function bridgeSourceLoaders(bridge: Bridge): SourceLoaders {
  return {
    listSeries: async () => {
      const reply = await bridge.call<SeriesListReply>("series.list", {});
      return Array.isArray(reply?.series) ? reply.series : [];
    },
  };
}
