import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const testDirectory = dirname(fileURLToPath(import.meta.url))
const repositoryRoot = resolve(testDirectory, '../../..')

test('frontend operations documents every stable launch and verification command', async () => {
  const operations = await readFile(resolve(repositoryRoot, 'docs/FRONTENDS-OPERATIONS.md'), 'utf8')

  for (const command of [
    'pnpm --dir apps doctor:web',
    'pnpm --dir apps doctor:desktop',
    'pnpm --dir apps dev:webui',
    'pnpm --dir apps dev:website',
    'pnpm --dir apps dev:desktop',
    'pnpm --dir apps stack:verify',
    'pnpm --dir apps test:tooling',
  ]) {
    assert.ok(operations.includes(command), `${command} is missing from frontend operations`)
  }
  assert.ok(operations.includes('http://127.0.0.1:4173/'))
  assert.ok(operations.includes('http://127.0.0.1:4174/'))
  assert.ok(operations.includes('https://diewehmut.github.io/Orchester/'))
  assert.ok(operations.includes('BASE_PATH=/Orchester/'))
  assert.match(operations, /non-zero exit/i)
})

test('toolchain guide names the machine-readable linker failures and supported repair', async () => {
  const guide = await readFile(resolve(repositoryRoot, 'docs/BUILD-TOOLCHAIN.md'), 'utf8')

  assert.ok(guide.includes('windows-linker-shadowed'))
  assert.ok(guide.includes('windows-msvc-compiler-missing'))
  assert.ok(guide.includes('pnpm --dir apps doctor:desktop'))
  assert.match(guide, /Desktop development with C\+\+/)
  assert.match(guide, /Developer PowerShell/)
})

test('apps readme routes contributors through the stable surface commands', async () => {
  const readme = await readFile(resolve(repositoryRoot, 'apps/README.md'), 'utf8')

  assert.ok(readme.includes('apps/stack.manifest.json'))
  assert.ok(readme.includes('pnpm run dev:webui'))
  assert.ok(readme.includes('pnpm run dev:website'))
  assert.ok(readme.includes('pnpm run dev:desktop'))
})
