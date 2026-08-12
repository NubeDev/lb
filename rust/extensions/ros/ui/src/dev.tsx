// Dev-only standalone harness: mount the remote into #root with an in-memory bridge so `pnpm dev`
// shows the page without the shell. NOT part of the federation expose — the shell uses `./remoteEntry`.
import { mount } from "@/remoteEntry";

const root = document.getElementById("root");
if (root) {
  mount(
    root,
    { workspace: "dev", isAdmin: true, route: "", onNavigate: () => {} },
    {
      // A dev bridge that resolves every call to an empty page — honest empty states, never
      // fabricated data. The real host bridge replaces this in the shell.
      call: async <T,>() => ({ items: [], next_cursor: null }) as T,
    },
  );
}
