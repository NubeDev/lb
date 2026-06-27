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
      // A dev bridge that resolves each granted verb to empty/ack data — honest empty states. Never
      // fabricated; the real host bridge replaces this in the shell. Shapes mirror the real verbs.
      call: async (tool: string) => {
        switch (tool) {
          case "series.latest":
            return { sample: null } as unknown;
          case "ingest.write":
            return { accepted: 0 } as unknown;
          case "outbox.status":
            return { pending: [], delivered: [], dead_lettered: [] } as unknown;
          case "inbox.list":
            return { items: [] } as unknown;
          case "inbox.resolve":
            return { ok: true } as unknown;
          default:
            return { series: [] } as unknown;
        }
      },
    },
  );
}
