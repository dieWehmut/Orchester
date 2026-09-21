import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile, stat } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const testDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(testDirectory, '../../..')

async function exists(path) {
  try {
    await stat(path)
    return true
  } catch {
    return false
  }
}

test('the workspace glob covers the deeper site payload', async () => {
  const workspace = await readFile(resolve(repositoryRoot, 'pnpm-workspace.yaml'), 'utf8')
  assert.match(workspace, /apps\/web\/site/)
})

test('the GitHub Pages site is the site surface of apps/web', async () => {
  assert.equal(
    await exists(resolve(repositoryRoot, 'apps/web/site/package.json')),
    true,
    'apps/web/site/package.json must exist',
  )
  assert.equal(
    await exists(resolve(repositoryRoot, 'apps/website')),
    false,
    'apps/website must move under apps/web/site',
  )
})
