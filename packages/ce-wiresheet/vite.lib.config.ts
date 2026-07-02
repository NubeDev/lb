import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import dts from 'vite-plugin-dts'
import { scopeWiresheetCss } from './scope-css'

// Library build — what hosts consume. Emits the CeEditor component (ESM + CJS), a
// bundled stylesheet (dist/ce-wiresheet.css, from src/index.ts's `import './wiresheet.css'`),
// and rolled-up types. React / React-DOM are PEER deps so the host provides one copy.
export default defineConfig({
  // `scopeWiresheetCss` LAST — it rewrites the FINAL emitted CSS (post-Tailwind) to scope the
  // `@theme` `:root,:host` token block and the vendored xyflow `.react-flow` rules under
  // `.ce-wiresheet`, so this library injects nothing global into a host document (slice-9).
  plugins: [react(), tailwindcss(), dts({ rollupTypes: true }), scopeWiresheetCss()],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    lib: {
      entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
      name: 'CeWiresheet',
      formats: ['es', 'cjs'],
      fileName: (fmt) => `ce-wiresheet.${fmt === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: ['react', 'react-dom', 'react/jsx-runtime', 'react-dom/client'],
      output: { assetFileNames: 'ce-wiresheet.[ext]', globals: { react: 'React', 'react-dom': 'ReactDOM' } },
    },
  },
})
