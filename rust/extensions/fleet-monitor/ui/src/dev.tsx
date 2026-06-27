// Dev-only standalone harness: mount the remote into #root with an in-memory bridge so `pnpm dev`
// shows the page without the shell. NOT part of the federation expose — the shell uses `./mount`.
import { mount } from "@/mount";

const root = document.getElementById("root");
if (root) {
  mount(
    root,
    { workspace: "dev" },
    {
      // A dev bridge that resolves the granted series verbs to empty data — honest empty states.
      // Never fabricated; the real host bridge replaces this in the shell.
      call: async () => [] as unknown,
    },
  );
}
