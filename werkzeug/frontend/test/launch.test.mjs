import test from 'node:test'
import assert from 'node:assert/strict'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import {
  createLaunchPlan,
  parseLaunchArguments,
  runLaunchPlan,
} from '../launch.mjs'
import { readStackManifest } from '../stack-manifest.mjs'

const testDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(testDirectory, '../../..')

test('launch plans use the declared packages and strict Vite ports', async () => {
  const manifest = await readStackManifest(repositoryRoot)

  assert.deepEqual(createLaunchPlan('webui', manifest, repositoryRoot), {
    surface: 'webui',
    command: 'pnpm',
    args: ['--dir', 'apps', '--filter', '@orchester/web', 'dev', '--', '--strictPort'],
    cwd: repositoryRoot,
    url: 'http://127.0.0.1:4173/',
    requiresDesktopDoctor: false,
  })
  assert.deepEqual(createLaunchPlan('website', manifest, repositoryRoot), {
    surface: 'website',
    command: 'pnpm',
    args: ['--dir', 'apps', '--filter', '@orchester/website', 'dev', '--', '--strictPort'],
    cwd: repositoryRoot,
    url: 'http://127.0.0.1:4174/',
    requiresDesktopDoctor: false,
  })
  assert.deepEqual(createLaunchPlan('desktop', manifest, repositoryRoot), {
    surface: 'desktop',
    command: 'pnpm',
    args: ['--dir', 'apps', '--filter', '@orchester/desktop', 'dev'],
    cwd: repositoryRoot,
    url: null,
    requiresDesktopDoctor: true,
  })
})

test('launch argument parsing is explicit and rejects unsupported surfaces', () => {
  assert.deepEqual(parseLaunchArguments(['webui', '--dry-run']), {
    surface: 'webui',
    dryRun: true,
    skipDoctor: false,
  })
  assert.deepEqual(parseLaunchArguments(['desktop', '--skip-doctor']), {
    surface: 'desktop',
    dryRun: false,
    skipDoctor: true,
  })
  assert.throws(() => parseLaunchArguments([]), /surface is required/)
  assert.throws(() => parseLaunchArguments(['pages']), /Unknown surface/)
  assert.throws(() => parseLaunchArguments(['webui', '--skip-doctor']), /desktop only/)
})

test('runLaunchPlan delegates to the injected process runner and propagates its exit code', async () => {
  const plan = {
    surface: 'webui',
    command: 'pnpm',
    args: ['--dir', 'apps'],
    cwd: repositoryRoot,
    url: 'http://127.0.0.1:4173/',
    requiresDesktopDoctor: false,
  }
  const calls = []

  const exitCode = await runLaunchPlan(plan, {
    spawnProcess(command, args, options) {
      calls.push({ command, args, options })
      return Promise.resolve(23)
    },
  })

  assert.equal(exitCode, 23)
  assert.deepEqual(calls, [{
    command: 'pnpm',
    args: ['--dir', 'apps'],
    options: { cwd: repositoryRoot, stdio: 'inherit', shell: process.platform === 'win32' },
  }])
})
