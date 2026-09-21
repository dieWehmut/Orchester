import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vitest/config'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '@orchester/ereignis': fileURLToPath(
        new URL('../../packages/ereignis/src/index.ts', import.meta.url),
      ),
    },
  },
  plugins: [vue()],
  test: {
    // The Pages site is a second payload under this package and runs its own
    // Vitest config. Keep this run to the WebUI sources so one package's suite
    // cannot silently adopt the other's tests.
    include: ['test/**/*.test.ts'],
    environment: 'jsdom',
    pool: 'threads',
    fileParallelism: false,
    maxWorkers: 1,
  },
})
