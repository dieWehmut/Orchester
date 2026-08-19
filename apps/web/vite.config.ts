import vue from '@vitejs/plugin-vue'
import { APPEARANCE_BOOTSTRAP_SCRIPT } from '@orchester/design/appearance-script'
import { defineConfig } from 'vite'

export default defineConfig({
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
