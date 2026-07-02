import { defineConfig } from 'vite'
import { fileURLToPath, URL } from 'node:url'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import dts from 'vite-plugin-dts'

// Library build — two entries, mirroring packages/source-picker's lib config but with a deliberate
// split that is the whole point of this package (genui-scope "Parse once, persist the IR"):
//   - `index`     → the RENDER stratum (ir/ + catalog/ + react/). Every viewer loads this; it carries
//                   NO parser and NO normalize — deterministic and immune to emission-format churn.
//   - `authoring` → the AUTHORING stratum (adapters/openui + normalize). The builder loads this only;
//                   it is the ONE place `@openuidev/lang-core` (the single external dep this scope adds)
//                   is pulled in. Externalized so it is not double-bundled and a viewer that imports
//                   only `.` never pays for the parser.
// React / react-dom are PEER deps (the host provides one copy). CSS is a single bundled `genui.css`
// (`cssCodeSplit:false`), emitted from `src/index.ts`'s `import './genui.css'`.
export default defineConfig({
  plugins: [react(), tailwindcss(), dts({ rollupTypes: true })],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    lib: {
      entry: {
        index: fileURLToPath(new URL('./src/index.ts', import.meta.url)),
        authoring: fileURLToPath(new URL('./src/authoring.ts', import.meta.url)),
      },
      name: 'NubeGenui',
      formats: ['es', 'cjs'],
      fileName: (fmt, name) => `${name}.${fmt === 'es' ? 'js' : 'cjs'}`,
    },
    rollupOptions: {
      external: [
        'react',
        'react-dom',
        'react/jsx-runtime',
        'react-dom/client',
        '@openuidev/lang-core',
      ],
      output: {
        assetFileNames: 'genui.[ext]',
        globals: { react: 'React', 'react-dom': 'ReactDOM' },
      },
    },
  },
})
