import vue from '@vitejs/plugin-vue'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    // Appearance state is a module singleton writing to a shared <html>, so two
    // files mutating it in the same worker would see each other's document.
    // One file per process keeps the reset in beforeEach honest.
    isolate: true,
  },
})
