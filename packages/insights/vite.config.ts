import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import dts from 'vite-plugin-dts'

// Library build — what hosts (the lb shell dashboard, a dashboard widget, a standalone extension UI)
// consume. Emits the insights machinery (ESM + CJS), a bundled scoped stylesheet
// (dist/insights.css from src/index.ts's `import './insights.css'`) and rolled-up types. React /
// React-DOM are PEER deps so the host provides one copy (an extension externalizes React to the shell
// import map — this must not bundle a second copy). Mirrors packages/source-picker's vite.config.ts.
export default defineConfig({
  plugins: [react(), tailwindcss(), dts({ rollupTypes: true })],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    lib: {
      entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
      name: 'NubeInsights',
      formats: ['es', 'cjs'],
      fileName: (fmt) => `insights.${fmt === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: ['react', 'react-dom', 'react/jsx-runtime', 'react-dom/client', 'lucide-react'],
      output: {
        assetFileNames: 'insights.[ext]',
        globals: { react: 'React', 'react-dom': 'ReactDOM' },
      },
    },
  },
})
