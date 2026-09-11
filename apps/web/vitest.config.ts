import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vitest/config'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '@orchester/ereignis': fileURLToPath(
        new URL('../ereignis/src/index.ts', import.meta.url),
      ),
    },
  },
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    pool: 'threads',
    fileParallelism: false,
    maxWorkers: 1,
  },
})
