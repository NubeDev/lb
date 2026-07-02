import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";

// Standalone playground app (docs/thecrew-scope.md). Not a library build — the lift
// into the framework extension happens by moving src/ folders, not by publishing.
export default defineConfig({
  plugins: [react(), tailwindcss()],
});
