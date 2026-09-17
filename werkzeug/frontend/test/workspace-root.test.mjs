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

test('the pnpm workspace root is the repository root, like the reference layout', async () => {
  const workspace = await readFile(resolve(repositoryRoot, 'pnpm-workspace.yaml'), 'utf8')
  assert.match(workspace, /apps\/\*/)

  const root = JSON.parse(await readFile(resolve(repositoryRoot, 'package.json'), 'utf8'))
  assert.equal(root.private, true)
  assert.equal(root.packageManager, 'pnpm@10.32.1')
  assert.equal(root.scripts['dev:webui'], 'node werkzeug/frontend/launch.mjs webui')
  assert.equal(root.scripts['dev:website'], 'node werkzeug/frontend/launch.mjs website')
  assert.equal(root.scripts['dev:desktop'], 'node werkzeug/frontend/launch.mjs desktop')
  assert.equal(root.scripts['doctor:desktop'], 'node werkzeug/frontend/doctor.mjs desktop')
  assert.equal(root.scripts['doctor:web'], 'node werkzeug/frontend/doctor.mjs web')
  assert.equal(root.scripts['test:tooling'], 'node --test werkzeug/frontend/test/*.test.mjs')
  assert.equal(root.scripts['stack:verify'], 'node werkzeug/frontend/stack-manifest.mjs')

  const npmrc = await readFile(resolve(repositoryRoot, '.npmrc'), 'utf8')
  assert.match(npmrc, /strict-peer-dependencies=false/)

  const tauri = JSON.parse(
    await readFile(resolve(repositoryRoot, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'),
  )
  assert.equal(tauri.build.beforeDevCommand.cwd, '../../..')
  assert.equal(tauri.build.beforeBuildCommand.cwd, '../../..')
})

test('the workspace files no longer live inside apps/', async () => {
  for (const path of [
    'apps/package.json',
    'apps/pnpm-workspace.yaml',
    'apps/pnpm-lock.yaml',
    'apps/.npmrc',
  ]) {
    assert.equal(
      await exists(resolve(repositoryRoot, path)),
      false,
      `${path} must move to the repository root`,
    )
  }
})
