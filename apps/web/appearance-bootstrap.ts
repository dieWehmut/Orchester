import { APPEARANCE_BOOTSTRAP_SCRIPT } from '@orchester/design/appearance-script'
import type { Plugin } from 'vite'

/** Blocking same-origin script applies saved appearance before the first paint. */
export function appearanceBootstrap(): Plugin {
  const filename = 'appearance-bootstrap.js'
  let base = '/'
  return {
    name: 'orchester-appearance-bootstrap',
    configResolved(config) { base = config.base },
    generateBundle() {
      this.emitFile({ type: 'asset', fileName: filename, source: APPEARANCE_BOOTSTRAP_SCRIPT })
    },
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        if (request.url?.split('?')[0] !== `${base}${filename}`) return next()
        response.setHeader('Content-Type', 'text/javascript; charset=utf-8')
        response.end(APPEARANCE_BOOTSTRAP_SCRIPT)
      })
    },
    transformIndexHtml: {
      order: 'post',
      handler() {
        return [{ tag: 'script', attrs: { src: `${base}${filename}` }, injectTo: 'head-prepend' }]
      },
    },
  }
}
