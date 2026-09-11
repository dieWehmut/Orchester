import test from 'node:test'
import assert from 'node:assert/strict'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  readStackManifest,
  validateStackManifest,
} from '../stack-manifest.mjs'

const testDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(testDirectory, '../../..')

test('stack manifest exposes stable launch and deployment metadata', async () => {
  const manifest = await readStackManifest(repositoryRoot)

  assert.equal(manifest.schemaVersion, 1)
  assert.deepEqual(Object.keys(manifest.surfaces), ['webui', 'website', 'desktop'])
  assert.deepEqual(manifest.surfaces.webui, {
    kind: 'vite',
    package: '@orchester/web',
    host: '127.0.0.1',
    port: 4173,
    url: 'http://127.0.0.1:4173/',
  })
  assert.deepEqual(manifest.surfaces.website, {
    kind: 'vite',
    package: '@orchester/website',
    host: '127.0.0.1',
    port: 4174,
    url: 'http://127.0.0.1:4174/',
  })
  assert.deepEqual(manifest.surfaces.desktop, {
    kind: 'tauri',
    package: '@orchester/desktop',
    frontend: 'webui',
  })
  assert.deepEqual(manifest.pages, {
    surface: 'website',
    basePath: '/Orchester/',
    url: 'https://diewehmut.github.io/Orchester/',
    artifactDirectory: 'apps/website/dist',
    workflow: '.github/workflows/pages.yml',
  })
  assert.deepEqual(manifest.toolchain, {
    node: '>=22.12.0',
    pagesNode: '24.8.0',
    pnpm: '10.32.1',
    rust: '>=1.80.0',
    windowsAbi: 'msvc',
  })
})

test('checked-in package, Vite, Tauri, and Pages settings match the manifest', async () => {
  const manifest = await readStackManifest(repositoryRoot)

  assert.deepEqual(await validateStackManifest(repositoryRoot, manifest), [])
})

test('repository validation reports port drift instead of accepting it', async () => {
  const manifest = structuredClone(await readStackManifest(repositoryRoot))
  manifest.surfaces.webui.port = 4999
  manifest.surfaces.webui.url = 'http://127.0.0.1:4999/'

  const errors = await validateStackManifest(repositoryRoot, manifest)

  assert.ok(errors.some((error) => error.includes('apps/web/vite.config.ts')))
  assert.ok(errors.some((error) => error.includes('Tauri devUrl')))
})
