import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const testDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(testDirectory, '../../..')

test('Pages validates the shared stack contract before building the website', async () => {
  const workflow = await readFile(resolve(repositoryRoot, '.github/workflows/pages.yml'), 'utf8')

  const installIndex = workflow.indexOf('pnpm --dir apps install --frozen-lockfile')
  const toolingIndex = workflow.indexOf('pnpm --dir apps test:tooling')
  const manifestIndex = workflow.indexOf('pnpm --dir apps stack:verify')
  const buildIndex = workflow.indexOf('pnpm --dir apps --filter @orchester/website build')

  assert.ok(installIndex >= 0)
  assert.ok(toolingIndex > installIndex)
  assert.ok(manifestIndex > toolingIndex)
  assert.ok(buildIndex > manifestIndex)
})

test('Pages path filters cover every checked-in input used by stack verification', async () => {
  const workflow = await readFile(resolve(repositoryRoot, '.github/workflows/pages.yml'), 'utf8')

  for (const path of [
    'Cargo.toml',
    'apps/stack.manifest.json',
    'apps/web/package.json',
    'apps/web/vite.config.ts',
    'apps/desktop/package.json',
    'apps/desktop/src-tauri/tauri.conf.json',
    'werkzeug/frontend/**',
  ]) {
    assert.match(workflow, new RegExp(`- ${path.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`))
  }
})
