import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import dts from 'vite-plugin-dts'

// Library build — what hosts (ce-wiresheet, lb, …) consume. Emits the NavRail
// component (ESM + CJS), a bundled stylesheet (dist/nav-rail.css, from src/index.ts's
// `import './nav-rail.css'`) and rolled-up types. React / React-DOM are PEER deps so the
// host provides one copy. Mirrors ce-wiresheet's own vite.lib.config.ts.
export default defineConfig({
  plugins: [react(), tailwindcss(), dts({ rollupTypes: true })],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    lib: {
      entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
      name: 'NubeNavRail',
      formats: ['es', 'cjs'],
      fileName: (fmt) => `nav-rail.${fmt === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: ['react', 'react-dom', 'react/jsx-runtime', 'react-dom/client'],
      output: { assetFileNames: 'nav-rail.[ext]', globals: { react: 'React', 'react-dom': 'ReactDOM' } },
    },
  },
})
