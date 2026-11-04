import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { appearanceBootstrap } from './appearance-bootstrap'
import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  resolve: {
    // The shared projection package is source-only until the workspace build
    // publishes package links. Keep Vite tests and dev mode deterministic.
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '@orchester/ereignis': fileURLToPath(
        new URL('../../packages/ereignis/src/index.ts', import.meta.url),
      ),
    },
  },
  plugins: [
    vue(),
    tailwindcss(),
    appearanceBootstrap(),
  ],
  server: {
    host: '127.0.0.1',
    port: 4173,
    strictPort: false,
  },
})
