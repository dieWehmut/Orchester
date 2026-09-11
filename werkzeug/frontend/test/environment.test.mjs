import test from 'node:test'
import assert from 'node:assert/strict'

import {
  diagnoseEnvironment,
  hasDiagnosticFailures,
} from '../environment.mjs'

function successful(stdout) {
  return { status: 0, stdout, stderr: '' }
}

function missing(command) {
  return { status: 1, stdout: '', stderr: `${command} was not found` }
}

function fixture(overrides = {}) {
  return {
    platform: 'win32',
    architecture: 'arm64',
    commands: {
      node: successful('v22.18.0'),
      pnpm: successful('10.32.1'),
      rustc: successful('rustc 1.96.1 (commit)\nhost: aarch64-pc-windows-msvc'),
      cargo: successful('cargo 1.96.1'),
      linker: successful('C:\\Program Files\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\14.44\\bin\\Hostarm64\\arm64\\link.exe'),
      compiler: successful('C:\\Program Files\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Tools\\MSVC\\14.44\\bin\\Hostarm64\\arm64\\cl.exe'),
      ...overrides,
    },
  }
}

test('healthy Windows desktop prerequisites have no failures', () => {
  const report = diagnoseEnvironment(fixture(), { profile: 'desktop' })

  assert.equal(report.profile, 'desktop')
  assert.equal(report.platform, 'win32')
  assert.equal(report.architecture, 'arm64')
  assert.equal(hasDiagnosticFailures(report), false)
  assert.ok(report.checks.every((check) => check.status !== 'fail'))
  assert.ok(report.checks.some((check) => check.id === 'windows-msvc-linker'))
  assert.equal(report.checks.find((check) => check.id === 'rustc').message, 'rustc 1.96.1 (commit)')
  assert.equal(report.checks.find((check) => check.id === 'cargo').message, 'cargo 1.96.1')
})

test('missing Rust tools fail the desktop profile but not the web profile', () => {
  const environment = fixture({ rustc: missing('rustc'), cargo: missing('cargo') })

  const desktop = diagnoseEnvironment(environment, { profile: 'desktop' })
  const web = diagnoseEnvironment(environment, { profile: 'web' })

  assert.deepEqual(
    desktop.checks.filter((check) => check.status === 'fail').map((check) => check.id),
    ['rustc', 'cargo'],
  )
  assert.equal(hasDiagnosticFailures(desktop), true)
  assert.equal(hasDiagnosticFailures(web), false)
})

test('ARM64 MSVC Rust rejects an MSYS linker and missing compiler with actionable codes', () => {
  const report = diagnoseEnvironment(
    fixture({
      linker: successful('D:\\software\\msys\\msys2\\usr\\bin\\link.exe'),
      compiler: missing('cl.exe'),
    }),
    { profile: 'desktop' },
  )

  const failures = report.checks.filter((check) => check.status === 'fail')
  assert.deepEqual(failures.map((check) => check.id), [
    'windows-linker-shadowed',
    'windows-msvc-compiler-missing',
  ])
  assert.match(failures[0].message, /MSYS/i)
  assert.match(failures[0].remediation, /Developer PowerShell/i)
  assert.match(failures[1].remediation, /Desktop development with C\+\+/i)
})

test('non-Windows desktop hosts do not require MSVC executables', () => {
  const environment = fixture()
  environment.platform = 'linux'
  environment.architecture = 'x64'
  environment.commands.linker = missing('link.exe')
  environment.commands.compiler = missing('cl.exe')

  const report = diagnoseEnvironment(environment, { profile: 'desktop' })

  assert.equal(hasDiagnosticFailures(report), false)
  assert.ok(!report.checks.some((check) => check.id.startsWith('windows-')))
})
