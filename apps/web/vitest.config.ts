import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    pool: 'threads',
    fileParallelism: false,
    maxWorkers: 1,
  },
})
