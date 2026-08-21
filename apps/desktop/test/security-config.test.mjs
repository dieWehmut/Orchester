import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const testDirectory = dirname(fileURLToPath(import.meta.url))
const tauriDirectory = resolve(testDirectory, '..', 'src-tauri')

async function readJson(relativePath) {
  const contents = await readFile(resolve(tauriDirectory, relativePath), 'utf8')
  return JSON.parse(contents)
}

function parseCsp(policy) {
  assert.equal(typeof policy, 'string')
  return new Map(
    policy
      .split(';')
      .map((directive) => directive.trim().split(/\s+/))
      .filter(([name]) => name)
      .map(([name, ...sources]) => [name, sources]),
  )
}

function assertStrictCsp(policy, { development }) {
  const directives = parseCsp(policy)
  assert.deepEqual(directives.get('default-src'), ["'self'"])
  assert.deepEqual(directives.get('script-src'), ["'self'"])
  assert.deepEqual(directives.get('object-src'), ["'none'"])
  assert.deepEqual(directives.get('base-uri'), ["'self'"])
  assert.deepEqual(directives.get('form-action'), ["'self'"])
  assert.deepEqual(directives.get('frame-ancestors'), ["'none'"])
  assert.deepEqual(directives.get('navigate-to'), ["'self'"])

  const connectSources = directives.get('connect-src')
  assert.ok(connectSources?.includes("'self'"))
  assert.ok(connectSources?.includes('ipc:'))
  assert.ok(connectSources?.includes('http://ipc.localhost'))
  if (development) {
    assert.ok(connectSources.includes('http://127.0.0.1:4173'))
    assert.ok(connectSources.includes('ws://127.0.0.1:4173'))
  } else {
    assert.ok(!connectSources.includes('http://127.0.0.1:4173'))
    assert.ok(!connectSources.includes('ws://127.0.0.1:4173'))
  }

  for (const sources of directives.values()) {
    assert.ok(!sources.includes('*'))
    assert.ok(!sources.includes("'unsafe-eval'"))
  }
}

test('desktop security configuration is explicit and deny-by-default', async () => {
  const [config, capability] = await Promise.all([
    readJson('tauri.conf.json'),
    readJson('capabilities/default.json'),
  ])

  const security = config.app.security
  assert.deepEqual(security.capabilities, ['default'])
  assert.equal(security.freezePrototype, true)
  assert.equal(security.dangerousDisableAssetCspModification, false)
  assert.equal(security.assetProtocol.enable, false)
  assert.deepEqual(security.assetProtocol.scope, [])

  assertStrictCsp(security.csp, { development: false })
  assertStrictCsp(security.devCsp, { development: true })

  assert.equal(capability.identifier, 'default')
  assert.deepEqual(capability.windows, ['main'])
  assert.equal(capability.local, true)
  assert.deepEqual(capability.remote?.urls, ['http://127.0.0.1:4173/*'])
  assert.deepEqual(capability.permissions, [
    'core:window:allow-close',
    'core:window:allow-minimize',
    'core:window:allow-toggle-maximize',
    'core:window:allow-start-dragging',
  ])
  assert.ok(capability.permissions.every((permission) => permission.startsWith('core:window:')))
  assert.ok(!capability.permissions.some((permission) => permission.startsWith('shell:')))
  assert.ok(!capability.permissions.some((permission) => permission.startsWith('opener:')))

  assert.equal(config.app.windows.length, 1)
  assert.equal(config.app.windows[0].label, 'main')
  assert.equal(config.app.windows[0].decorations, false)
  assert.equal(config.app.windows[0].shadow, true)
  assert.equal(config.app.windows[0].devtools, false)
  assert.deepEqual(config.bundle.icon, ['icons/icon.png', 'icons/icon.ico'])
})
