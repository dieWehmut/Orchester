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

test('the local CLI is the cli surface of apps/', async () => {
  assert.equal(
    await exists(resolve(repositoryRoot, 'apps/cli/package.json')),
    true,
    'apps/cli/package.json must exist',
  )
  assert.equal(
    await exists(resolve(repositoryRoot, 'apps/cli/bin/orchester.cjs')),
    true,
    'apps/cli/bin/orchester.cjs must exist',
  )
  assert.equal(
    await exists(resolve(repositoryRoot, 'npm/cli')),
    false,
    'npm/cli must move to apps/cli',
  )
})
