// @vitest-environment node
import { describe, expect, it } from 'vitest'
import { build, createServer } from 'vite'
import { appearanceBootstrap } from '../appearance-bootstrap'
import { APPEARANCE_BOOTSTRAP_SCRIPT } from '@orchester/design/appearance-script'

describe('appearance startup under the embedded CSP', () => {
  it.each(['/', '/orchester/'])('emits a blocking same-origin script before the app for base %s', async (base) => {
    const result = await build({
      configFile: false,
      base,
      plugins: [appearanceBootstrap()],
      build: { write: false, minify: false, rollupOptions: { input: 'index.html', external: ['/src/main.ts'] } },
      logLevel: 'silent',
    })
    const bundle = Array.isArray(result) ? result[0] : result
    if (!bundle || !('output' in bundle)) throw new Error('Expected output bundle')
    const html = bundle.output.find(item => item.fileName === 'index.html')
    const script = bundle.output.find(item => item.fileName === 'appearance-bootstrap.js')
    expect(html?.type).toBe('asset')
    expect(script?.type === 'asset' && script.source).toBe(APPEARANCE_BOOTSTRAP_SCRIPT)
    const markup = html?.type === 'asset' ? String(html.source) : ''
    expect(markup).toContain(`<script src="${base}appearance-bootstrap.js"></script>`)
    expect(markup).toContain('<script type="module"')
    expect(markup.indexOf('appearance-bootstrap.js')).toBeLessThan(markup.indexOf('<script type="module"'))
    expect(markup).not.toContain(APPEARANCE_BOOTSTRAP_SCRIPT)
  })
  it('serves the same script during development under a nested base', async () => {
    const server = await createServer({ configFile: false, base: '/orchester/', plugins: [appearanceBootstrap()], server: { host: '127.0.0.1', port: 0 } })
    try {
      await server.listen()
      const address = server.httpServer!.address()
      if (!address || typeof address === 'string') throw new Error('Expected TCP server')
      const response = await fetch(`http://127.0.0.1:${address.port}/orchester/appearance-bootstrap.js`)
      expect(response.status).toBe(200)
      expect(response.headers.get('content-type')).toContain('javascript')
      expect(await response.text()).toBe(APPEARANCE_BOOTSTRAP_SCRIPT)
    } finally { await server.close() }
  })
})
