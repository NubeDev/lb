import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import dts from 'vite-plugin-dts'

// Library build — what hosts (lb shell, the thecrew extension, …) consume. Emits the picker (ESM +
// CJS), a bundled stylesheet (dist/source-picker.css from src/index.ts's `import './source-picker.css'`)
// and rolled-up types. React / React-DOM are PEER deps so the host provides one copy (an extension
// externalizes React to the shell import map — this must not bundle a second copy). Mirrors
// packages/nav-rail's vite.config.ts.
export default defineConfig({
  plugins: [react(), tailwindcss(), dts({ rollupTypes: true })],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    lib: {
      entry: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
      name: 'NubeSourcePicker',
      formats: ['es', 'cjs'],
      fileName: (fmt) => `source-picker.${fmt === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: [
        'react',
        'react-dom',
        'react/jsx-runtime',
        'react-dom/client',
        '@radix-ui/react-collapsible',
        'lucide-react',
      ],
      output: { assetFileNames: 'source-picker.[ext]', globals: { react: 'React', 'react-dom': 'ReactDOM' } },
    },
  },
})
