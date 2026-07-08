import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

// The Tailwind v4 vite plugin processes `@import "tailwindcss/..."` in the CSS entry. The template
// imports utilities + theme WITHOUT preflight (a federated widget must NOT reset the host's base
// styles — the ce-wiresheet scope's load-bearing rule).
export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    lib: {
      entry: "src/remoteEntry.ts",
      formats: ["es"],
      fileName: () => "remoteEntry.js"
    },
    rollupOptions: {
      // Only react/react-dom are shared externals (the shell provides them). recharts and all
      // other deps are BUNDLED into the extension's remoteEntry.js — the shell does NOT provide
      // them, so marking them external produces an unresolvable bare import at runtime.
      external: ["react", "react-dom", "react/jsx-runtime"]
    }
  }
});
