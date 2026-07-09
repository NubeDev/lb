import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// The dev server proxies `/convert` to the local Rust seam (the app binary at
// `GRAFANA_CONV_ADDR`, default `127.0.0.1:7878`). Run `cargo run -p
// grafana-conv-app` in one terminal, then `pnpm dev` here.
export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  server: { proxy: { "/convert": "http://127.0.0.1:7878" } },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    css: true,
  },
});
