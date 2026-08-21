import test from 'node:test'
import assert from 'node:assert/strict'

import { runDoctor } from '../doctor.mjs'

test('doctor returns non-zero and preserves structured diagnostics when required checks fail', async () => {
  const output = []
  const report = {
    profile: 'desktop',
    platform: 'win32',
    architecture: 'arm64',
    checks: [{
      id: 'windows-linker-shadowed',
      status: 'fail',
      message: 'MSYS link.exe shadows the Microsoft linker.',
      remediation: 'Open Developer PowerShell.',
    }],
  }

  const exitCode = await runDoctor(['desktop', '--json'], {
    readManifest: async () => ({
      toolchain: { node: '>=22.12.0', pnpm: '10.32.1', rust: '>=1.80.0' },
    }),
    inspect: async () => report,
    write: (chunk) => output.push(chunk),
  })

  assert.equal(exitCode, 1)
  assert.deepEqual(JSON.parse(output.join('')), report)
})

test('doctor returns zero for a warning-free web report', async () => {
  const output = []
  const exitCode = await runDoctor(['web'], {
    readManifest: async () => ({
      toolchain: { node: '>=22.12.0', pnpm: '10.32.1', rust: '>=1.80.0' },
    }),
    inspect: async () => ({
      profile: 'web',
      platform: 'win32',
      architecture: 'arm64',
      checks: [{ id: 'node', status: 'pass', message: 'Node.js v24.8.0' }],
    }),
    write: (chunk) => output.push(chunk),
  })

  assert.equal(exitCode, 0)
  assert.match(output.join(''), /PASS.*node.*Node\.js v24\.8\.0/)
})
