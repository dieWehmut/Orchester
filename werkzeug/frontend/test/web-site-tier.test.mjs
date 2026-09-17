import test from 'node:test'
import assert from 'node:assert/strict'
import { stat } from 'node:fs/promises'
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
