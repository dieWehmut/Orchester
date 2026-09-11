import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const testDirectory = dirname(fileURLToPath(import.meta.url))
const packagePath = resolve(testDirectory, '../../../apps/package.json')

test('apps package exposes unambiguous surface and doctor commands', async () => {
  const manifest = JSON.parse(await readFile(packagePath, 'utf8'))
  assert.equal(manifest.scripts['dev:webui'], 'node ../werkzeug/frontend/launch.mjs webui')
  assert.equal(manifest.scripts['dev:website'], 'node ../werkzeug/frontend/launch.mjs website')
  assert.equal(manifest.scripts['dev:desktop'], 'node ../werkzeug/frontend/launch.mjs desktop')
  assert.equal(manifest.scripts['doctor:desktop'], 'node ../werkzeug/frontend/doctor.mjs desktop')
  assert.equal(manifest.scripts['doctor:web'], 'node ../werkzeug/frontend/doctor.mjs web')
  assert.equal(Object.hasOwn(manifest.scripts, 'doctor'), false)
  assert.equal(manifest.scripts['test:tooling'], 'node --test ../werkzeug/frontend/test/*.test.mjs')
  assert.equal(
    manifest.scripts.test,
    'pnpm run test:tooling && pnpm -r test && pnpm --filter @orchester/desktop test:security',
  )
})
