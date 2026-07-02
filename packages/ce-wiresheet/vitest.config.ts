import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'

// Tests run under jsdom (the rendering tests need a DOM; the pure-logic tests don't
// care). CSS imports are stubbed by vitest, so no Tailwind in the test build.
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    include: ['src/**/*.test.{ts,tsx}'],
    globals: true,
  },
})
