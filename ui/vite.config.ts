import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "node:path";

// Vite + React. Tauri serves this build in the desktop shell; the same build is served to
// browsers via the SSE gateway at S3. The `@` alias mirrors the src root (FILE-LAYOUT).
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "src") },
  },
  // Tauri expects a fixed dev port and no clearing of the screen.
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
  },
});
