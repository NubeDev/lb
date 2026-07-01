import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import dts from 'vite-plugin-dts'

// Library build — what hosts (lb `ui`, other Nube apps) consume. Emits the Panel
// component set (ESM + CJS), a bundled stylesheet (dist/panel.css, from src/index.ts's
// `import './panel.css'`) and rolled-up types. React / React-DOM are PEER deps so the
// host provides one copy. @nube/nav-rail is bundled in (an internal dependency — the
// panel's section rail is its NavMenu). Mirrors @nube/nav-rail's own vite.config.ts.
export default defineConfig({
  plugins: [react(), tailwindcss(), dts({ rollupTypes: true })],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    lib: {
      entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
      name: 'NubePanel',
      formats: ['es', 'cjs'],
      fileName: (fmt) => `panel.${fmt === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: ['react', 'react-dom', 'react/jsx-runtime', 'react-dom/client'],
      output: { assetFileNames: 'panel.[ext]', globals: { react: 'React', 'react-dom': 'ReactDOM' } },
    },
  },
})
