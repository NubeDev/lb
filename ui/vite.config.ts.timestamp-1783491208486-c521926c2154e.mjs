// vite.config.ts
import { defineConfig } from "file:///home/user/code/rust/lb/node_modules/.pnpm/vite@5.4.21_@types+node@26.0.1_lightningcss@1.32.0_terser@5.48.0/node_modules/vite/dist/node/index.js";
import react from "file:///home/user/code/rust/lb/node_modules/.pnpm/@vitejs+plugin-react@4.7.0_vite@5.4.21_@types+node@26.0.1_lightningcss@1.32.0_terser@5.48.0_/node_modules/@vitejs/plugin-react/dist/index.js";
import tailwindcss from "file:///home/user/code/rust/lb/node_modules/.pnpm/@tailwindcss+vite@4.3.2_vite@5.4.21_@types+node@26.0.1_lightningcss@1.32.0_terser@5.48.0_/node_modules/@tailwindcss/vite/dist/index.mjs";
import path from "node:path";
var __vite_injected_original_dirname = "/home/user/code/rust/lb/ui";
var vite_config_default = defineConfig(({ command }) => {
  const nodeEnv = JSON.stringify(command === "build" ? "production" : "development");
  return {
    plugins: [react(), tailwindcss()],
    define: {
      "process.env.NODE_ENV": nodeEnv
    },
    optimizeDeps: {
      esbuildOptions: {
        define: {
          "process.env.NODE_ENV": nodeEnv
        }
      }
    },
    // esnext: extension remotes may use top-level await; keep the host build modern to match.
    build: { target: "esnext" },
    resolve: {
      alias: { "@": path.resolve(__vite_injected_original_dirname, "src") }
    },
    // Tauri expects a fixed dev port and no clearing of the screen.
    clearScreen: false,
    server: { port: 5173, strictPort: true },
    test: {
      environment: "jsdom",
      globals: true,
      setupFiles: ["./src/test/setup.ts"],
      // The real-gateway tests (`*.gateway.test.ts[x]`) need a spawned node; they run under their own
      // `vitest.gateway.config.ts` (`pnpm test:gateway`), not this default suite.
      include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
      exclude: ["**/node_modules/**", "e2e/**", "**/*.gateway.test.ts", "**/*.gateway.test.tsx"]
    }
  };
});
export {
  vite_config_default as default
};
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcudHMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCIvaG9tZS91c2VyL2NvZGUvcnVzdC9sYi91aVwiO2NvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9maWxlbmFtZSA9IFwiL2hvbWUvdXNlci9jb2RlL3J1c3QvbGIvdWkvdml0ZS5jb25maWcudHNcIjtjb25zdCBfX3ZpdGVfaW5qZWN0ZWRfb3JpZ2luYWxfaW1wb3J0X21ldGFfdXJsID0gXCJmaWxlOi8vL2hvbWUvdXNlci9jb2RlL3J1c3QvbGIvdWkvdml0ZS5jb25maWcudHNcIjtpbXBvcnQgeyBkZWZpbmVDb25maWcgfSBmcm9tIFwidml0ZVwiO1xuaW1wb3J0IHJlYWN0IGZyb20gXCJAdml0ZWpzL3BsdWdpbi1yZWFjdFwiO1xuaW1wb3J0IHRhaWx3aW5kY3NzIGZyb20gXCJAdGFpbHdpbmRjc3Mvdml0ZVwiO1xuaW1wb3J0IHBhdGggZnJvbSBcIm5vZGU6cGF0aFwiO1xuXG4vLyBWaXRlICsgUmVhY3QuIFRhdXJpIHNlcnZlcyB0aGlzIGJ1aWxkIGluIHRoZSBkZXNrdG9wIHNoZWxsOyB0aGUgc2FtZSBidWlsZCBpcyBzZXJ2ZWQgdG8gYnJvd3NlcnMgdmlhXG4vLyB0aGUgU1NFIGdhdGV3YXkgYXQgUzMuIFRoZSBgQGAgYWxpYXMgbWlycm9ycyB0aGUgc3JjIHJvb3QgKEZJTEUtTEFZT1VUKS5cbi8vXG4vLyBFeHRlbnNpb24gZmVkZXJhdGlvbiAodWktZmVkZXJhdGlvbiBzY29wZSk6IHRoZSBzaGVsbCBwdWJsaXNoZXMgaXRzIGByZWFjdGAvYHJlYWN0LWRvbWAvXG4vLyBgcmVhY3QtZG9tL2NsaWVudGAvYHJlYWN0L2pzeC1ydW50aW1lYCBhcyBTSU5HTEVUT05TIG9uIGBnbG9iYWxUaGlzLl9fbGIqYCAoc3JjL2ZlYXR1cmVzL2V4dC1ob3N0L1xuLy8gc2luZ2xldG9ucy50cykgYW5kIGRlY2xhcmVzIGFuIGltcG9ydCBtYXAgaW4gYGluZGV4Lmh0bWxgIHRoYXQgbWFwcyB0aG9zZSBiYXJlIHNwZWNpZmllcnMgdG8gdGhlXG4vLyBgL3NoaW1zLyoubWpzYCByZS1leHBvcnRlcnMuIEFuIGV4dGVuc2lvbiBgcmVtb3RlRW50cnkuanNgIFx1MjAxNCBidWlsdCBhcyBhbiBFU00gbGliIHdpdGggdGhvc2UgbW9kdWxlc1xuLy8gZXh0ZXJuYWxpc2VkIFx1MjAxNCBpcyBkeW5hbWljLWltcG9ydGVkIGF0IHJ1bnRpbWUgYnkgZ2F0ZXdheSBVUkwgKGV4dC1ob3N0L2ZlZGVyYXRpb24udHMpIGFuZCByZW5kZXJzXG4vLyBpbi1wcm9jZXNzIGFnYWluc3QgdGhlIGhvc3QncyBTSU5HTEUgUmVhY3QuIE5vIGJ1aWxkLXRpbWUgZmVkZXJhdGlvbiBwbHVnaW4gaXMgbmVlZGVkOyB0aGlzIHJlcGxhY2VzXG4vLyBgQG9yaWdpbmpzL3ZpdGUtcGx1Z2luLWZlZGVyYXRpb25gLCB3aG9zZSBkeW5hbWljLXJlbW90ZSBzaGFyZSBzY29wZSBzaGlwcGVkIGEgc2Vjb25kIFJlYWN0IGFuZCBicm9rZVxuLy8gaG9va3MgKFwiSW52YWxpZCBob29rIGNhbGxcIikuIFNlZSBkZWJ1Z2dpbmcvZXh0ZW5zaW9ucy9mZWRlcmF0ZWQtcmVtb3RlLWZhaWxzLWluLWRldi1zZXJ2ZXIubWQuXG5leHBvcnQgZGVmYXVsdCBkZWZpbmVDb25maWcoKHsgY29tbWFuZCB9KSA9PiB7XG4gIGNvbnN0IG5vZGVFbnYgPSBKU09OLnN0cmluZ2lmeShjb21tYW5kID09PSBcImJ1aWxkXCIgPyBcInByb2R1Y3Rpb25cIiA6IFwiZGV2ZWxvcG1lbnRcIik7XG5cbiAgcmV0dXJuIHtcbiAgICBwbHVnaW5zOiBbcmVhY3QoKSwgdGFpbHdpbmRjc3MoKV0sXG4gICAgZGVmaW5lOiB7XG4gICAgICBcInByb2Nlc3MuZW52Lk5PREVfRU5WXCI6IG5vZGVFbnYsXG4gICAgfSxcbiAgICBvcHRpbWl6ZURlcHM6IHtcbiAgICAgIGVzYnVpbGRPcHRpb25zOiB7XG4gICAgICAgIGRlZmluZToge1xuICAgICAgICAgIFwicHJvY2Vzcy5lbnYuTk9ERV9FTlZcIjogbm9kZUVudixcbiAgICAgICAgfSxcbiAgICAgIH0sXG4gICAgfSxcbiAgICAvLyBlc25leHQ6IGV4dGVuc2lvbiByZW1vdGVzIG1heSB1c2UgdG9wLWxldmVsIGF3YWl0OyBrZWVwIHRoZSBob3N0IGJ1aWxkIG1vZGVybiB0byBtYXRjaC5cbiAgICBidWlsZDogeyB0YXJnZXQ6IFwiZXNuZXh0XCIgfSxcbiAgICByZXNvbHZlOiB7XG4gICAgICBhbGlhczogeyBcIkBcIjogcGF0aC5yZXNvbHZlKF9fZGlybmFtZSwgXCJzcmNcIikgfSxcbiAgICB9LFxuICAgIC8vIFRhdXJpIGV4cGVjdHMgYSBmaXhlZCBkZXYgcG9ydCBhbmQgbm8gY2xlYXJpbmcgb2YgdGhlIHNjcmVlbi5cbiAgICBjbGVhclNjcmVlbjogZmFsc2UsXG4gICAgc2VydmVyOiB7IHBvcnQ6IDUxNzMsIHN0cmljdFBvcnQ6IHRydWUgfSxcbiAgICB0ZXN0OiB7XG4gICAgICBlbnZpcm9ubWVudDogXCJqc2RvbVwiLFxuICAgICAgZ2xvYmFsczogdHJ1ZSxcbiAgICAgIHNldHVwRmlsZXM6IFtcIi4vc3JjL3Rlc3Qvc2V0dXAudHNcIl0sXG4gICAgICAvLyBUaGUgcmVhbC1nYXRld2F5IHRlc3RzIChgKi5nYXRld2F5LnRlc3QudHNbeF1gKSBuZWVkIGEgc3Bhd25lZCBub2RlOyB0aGV5IHJ1biB1bmRlciB0aGVpciBvd25cbiAgICAgIC8vIGB2aXRlc3QuZ2F0ZXdheS5jb25maWcudHNgIChgcG5wbSB0ZXN0OmdhdGV3YXlgKSwgbm90IHRoaXMgZGVmYXVsdCBzdWl0ZS5cbiAgICAgIGluY2x1ZGU6IFtcInNyYy8qKi8qLnRlc3QudHNcIiwgXCJzcmMvKiovKi50ZXN0LnRzeFwiXSxcbiAgICAgIGV4Y2x1ZGU6IFtcIioqL25vZGVfbW9kdWxlcy8qKlwiLCBcImUyZS8qKlwiLCBcIioqLyouZ2F0ZXdheS50ZXN0LnRzXCIsIFwiKiovKi5nYXRld2F5LnRlc3QudHN4XCJdLFxuICAgIH0sXG4gIH07XG59KTtcbiJdLAogICJtYXBwaW5ncyI6ICI7QUFBZ1EsU0FBUyxvQkFBb0I7QUFDN1IsT0FBTyxXQUFXO0FBQ2xCLE9BQU8saUJBQWlCO0FBQ3hCLE9BQU8sVUFBVTtBQUhqQixJQUFNLG1DQUFtQztBQWdCekMsSUFBTyxzQkFBUSxhQUFhLENBQUMsRUFBRSxRQUFRLE1BQU07QUFDM0MsUUFBTSxVQUFVLEtBQUssVUFBVSxZQUFZLFVBQVUsZUFBZSxhQUFhO0FBRWpGLFNBQU87QUFBQSxJQUNMLFNBQVMsQ0FBQyxNQUFNLEdBQUcsWUFBWSxDQUFDO0FBQUEsSUFDaEMsUUFBUTtBQUFBLE1BQ04sd0JBQXdCO0FBQUEsSUFDMUI7QUFBQSxJQUNBLGNBQWM7QUFBQSxNQUNaLGdCQUFnQjtBQUFBLFFBQ2QsUUFBUTtBQUFBLFVBQ04sd0JBQXdCO0FBQUEsUUFDMUI7QUFBQSxNQUNGO0FBQUEsSUFDRjtBQUFBO0FBQUEsSUFFQSxPQUFPLEVBQUUsUUFBUSxTQUFTO0FBQUEsSUFDMUIsU0FBUztBQUFBLE1BQ1AsT0FBTyxFQUFFLEtBQUssS0FBSyxRQUFRLGtDQUFXLEtBQUssRUFBRTtBQUFBLElBQy9DO0FBQUE7QUFBQSxJQUVBLGFBQWE7QUFBQSxJQUNiLFFBQVEsRUFBRSxNQUFNLE1BQU0sWUFBWSxLQUFLO0FBQUEsSUFDdkMsTUFBTTtBQUFBLE1BQ0osYUFBYTtBQUFBLE1BQ2IsU0FBUztBQUFBLE1BQ1QsWUFBWSxDQUFDLHFCQUFxQjtBQUFBO0FBQUE7QUFBQSxNQUdsQyxTQUFTLENBQUMsb0JBQW9CLG1CQUFtQjtBQUFBLE1BQ2pELFNBQVMsQ0FBQyxzQkFBc0IsVUFBVSx3QkFBd0IsdUJBQXVCO0FBQUEsSUFDM0Y7QUFBQSxFQUNGO0FBQ0YsQ0FBQzsiLAogICJuYW1lcyI6IFtdCn0K
