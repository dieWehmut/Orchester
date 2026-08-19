import vue from '@vitejs/plugin-vue'
import { APPEARANCE_BOOTSTRAP_SCRIPT } from '@orchester/design/appearance-script'
import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  resolve: {
    // The shared projection package is source-only until the workspace build
    // publishes package links. Keep Vite tests and dev mode deterministic.
    alias: {
      '@orchester/ereignis': fileURLToPath(
        new URL('../ereignis/src/index.ts', import.meta.url),
      ),
    },
  },
  plugins: [
    vue(),
    {
      name: 'orchester-appearance-bootstrap',
      transformIndexHtml: {
        order: 'pre',
        handler() {
          return [
            {
              tag: 'script',
              children: APPEARANCE_BOOTSTRAP_SCRIPT,
              injectTo: 'head-prepend',
            },
          ]
        },
      },
    },
  ],
  server: {
    host: '127.0.0.1',
    port: 4173,
    strictPort: false,
  },
})
