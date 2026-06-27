// Dev-only standalone harness: mount the remote into #root with an in-memory bridge so `pnpm dev`
// shows the page without the shell. NOT part of the federation expose — the shell uses `./mount`.
import { mount } from "@/mount";
// The standalone dev harness imports the page CSS directly (production injects it via remoteEntry.ts).
import "@/styles/tokens.css";

const root = document.getElementById("root");
if (root) {
  mount(
    root,
    { workspace: "dev" },
    {
      // A dev bridge that resolves the granted series verbs to empty data — honest empty states.
      // Never fabricated; the real host bridge replaces this in the shell. Shapes mirror the real
      // verbs: series.find → { series: [] }, series.latest → { sample: null }.
      call: async (tool: string) =>
        (tool === "series.latest" ? { sample: null } : { series: [] }) as unknown,
    },
  );
}
