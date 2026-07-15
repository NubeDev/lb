import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import dts from 'vite-plugin-dts'

// Library build — what hosts (lb `ui`, rubix-ai, other Nube apps) consume. Emits the grid
// core (ESM + CJS), a bundled stylesheet (dist/dashboard.css, from src/index.ts's
// `import './dashboard.css'`) and rolled-up types. React / React-DOM are PEER deps so the
// host provides one copy; react-grid-layout + react-resizable are BUNDLED (an internal
// mechanism, not a contract the host should have to install). Mirrors @nube/panel's build.
export default defineConfig({
  plugins: [react(), dts({ rollupTypes: true })],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    lib: {
      entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
      name: 'NubeDashboard',
      formats: ['es', 'cjs'],
      fileName: (fmt) => `dashboard.${fmt === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: ['react', 'react-dom', 'react/jsx-runtime', 'react-dom/client'],
      output: { assetFileNames: 'dashboard.[ext]', globals: { react: 'React', 'react-dom': 'ReactDOM' } },
    },
  },
})
