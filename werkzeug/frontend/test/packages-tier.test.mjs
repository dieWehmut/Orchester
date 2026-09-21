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

test('the shared TypeScript packages live in a root packages/ tier', async () => {
  for (const name of ['design', 'ereignis', 'protokoll']) {
    assert.equal(
      await exists(resolve(repositoryRoot, `packages/${name}/package.json`)),
      true,
      `packages/${name}/package.json must exist`,
    )
    assert.equal(
      await exists(resolve(repositoryRoot, `apps/${name}`)),
      false,
      `apps/${name} must move to packages/${name}`,
    )
  }
})
