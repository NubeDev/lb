import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import dts from 'vite-plugin-dts'

// Library build — what hosts consume. Emits the CeEditor component (ESM + CJS), a
// bundled stylesheet (dist/ce-wiresheet.css, from src/index.ts's `import './wiresheet.css'`),
// and rolled-up types. React / React-DOM are PEER deps so the host provides one copy.
export default defineConfig({
  plugins: [react(), tailwindcss(), dts({ rollupTypes: true })],
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
