import { resolve } from 'node:path'

import vue from '@vitejs/plugin-vue'
import { APPEARANCE_BOOTSTRAP_SCRIPT } from '@orchester/design/appearance-script'
import { defineConfig, type ResolvedConfig } from 'vite'

import { normalizeBasePath } from './src/base-path'
import { copyIndexTo404 } from './src/build/copy-404'

let outputDirectory = ''

export default defineConfig({
  base: normalizeBasePath(process.env.BASE_PATH ?? process.env.VITE_BASE_PATH),
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
    {
      name: 'orchester-pages-copy-404',
      configResolved(config: ResolvedConfig) {
        outputDirectory = resolve(config.root, config.build.outDir)
      },
      async closeBundle() {
        await copyIndexTo404(outputDirectory)
      },
    },
  ],
  server: {
    host: '127.0.0.1',
    port: 4174,
    strictPort: false,
  },
})
