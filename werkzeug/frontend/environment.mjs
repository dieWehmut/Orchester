import { spawnSync } from 'node:child_process'

function check(id, status, message, remediation) {
  return remediation ? { id, status, message, remediation } : { id, status, message }
}

function versionTuple(version) {
  const match = String(version).match(/(\d+)\.(\d+)\.(\d+)/)
  return match ? match.slice(1).map(Number) : null
}

function meetsMinimum(actual, minimum) {
  const actualParts = versionTuple(actual)
  const minimumParts = versionTuple(minimum)
  if (!actualParts || !minimumParts) return false
  for (let index = 0; index < minimumParts.length; index += 1) {
    if (actualParts[index] > minimumParts[index]) return true
    if (actualParts[index] < minimumParts[index]) return false
  }
  return true
}

function firstLine(result) {
  return result.stdout.trim().split(/\r?\n/)[0] ?? ''
}

function toolCheck(id, label, result, minimum, remediation) {
  if (result.status !== 0) return check(id, 'fail', `${label} is not available.`, remediation)
  const version = firstLine(result)
  if (minimum && !meetsMinimum(version, minimum)) {
    return check(id, 'fail', `${label} ${version || 'unknown'} is older than ${minimum}.`, remediation)
  }
  const displayVersion = version.replace(new RegExp(`^${label}\\s+`, 'i'), '')
  return check(id, 'pass', `${label} ${displayVersion}`)
}

function isMsvcRustHost(result) {
  return result.status === 0 && /host:\s+\S+-pc-windows-msvc/i.test(result.stdout)
}

function commandPaths(result) {
  return result.stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean)
}

function windowsDesktopChecks(environment) {
  const { linker, compiler, rustc } = environment.commands
  if (!isMsvcRustHost(rustc)) {
    return [check(
      'windows-rust-abi',
      'fail',
      'Rust does not target the Windows MSVC ABI.',
      'Install and select stable-aarch64-pc-windows-msvc or stable-x86_64-pc-windows-msvc.',
    )]
  }

  const checks = []
  const linkerPaths = commandPaths(linker)
  if (linker.status !== 0 || linkerPaths.length === 0) {
    checks.push(check(
      'windows-msvc-linker-missing',
      'fail',
      'Microsoft link.exe is not available in PATH.',
      'Install Visual Studio Build Tools and open its architecture-matched Developer PowerShell.',
    ))
  } else if (/[/\\](msys|mingw)[/\\]/i.test(linkerPaths[0])) {
    checks.push(check(
      'windows-linker-shadowed',
      'fail',
      `MSYS/MinGW link.exe shadows the Microsoft linker: ${linkerPaths[0]}`,
      'Open an architecture-matched Visual Studio Developer PowerShell and remove MSYS/MinGW bin directories from PATH.',
    ))
  } else if (!/Microsoft Visual Studio|Windows Kits/i.test(linkerPaths[0])) {
    checks.push(check(
      'windows-linker-unknown',
      'fail',
      `link.exe provenance is not recognized as MSVC: ${linkerPaths[0]}`,
      'Use the link.exe exported by an architecture-matched Visual Studio Developer PowerShell.',
    ))
  } else {
    checks.push(check('windows-msvc-linker', 'pass', `Microsoft linker: ${linkerPaths[0]}`))
  }

  const compilerPaths = commandPaths(compiler)
  if (compiler.status !== 0 || compilerPaths.length === 0) {
    checks.push(check(
      'windows-msvc-compiler-missing',
      'fail',
      'MSVC cl.exe is not available in PATH.',
      'Install Visual Studio Build Tools with Desktop development with C++, the matching ARM64/x64 MSVC tools, and Windows SDK; then open Developer PowerShell.',
    ))
  } else {
    checks.push(check('windows-msvc-compiler', 'pass', `MSVC compiler: ${compilerPaths[0]}`))
  }

  return checks
}

export function diagnoseEnvironment(environment, options = {}) {
  const profile = options.profile ?? 'desktop'
  const minimums = options.minimums ?? { node: '22.12.0', pnpm: '10.32.1', rust: '1.80.0' }
  const checks = [
    toolCheck('node', 'Node.js', environment.commands.node, minimums.node, 'Install the Node.js version declared by apps/stack.manifest.json.'),
    toolCheck('pnpm', 'pnpm', environment.commands.pnpm, minimums.pnpm, 'Install pnpm with Corepack or npm using the version declared by apps/package.json.'),
  ]

  if (profile === 'desktop') {
    checks.push(
      toolCheck('rustc', 'rustc', environment.commands.rustc, minimums.rust, 'Install the stable Rust MSVC toolchain with rustup.'),
      toolCheck('cargo', 'cargo', environment.commands.cargo, minimums.rust, 'Install Cargo through rustup.'),
    )
    if (environment.platform === 'win32' && checks.slice(-2).every((item) => item.status === 'pass')) {
      checks.push(...windowsDesktopChecks(environment))
    }
  }

  return {
    profile,
    platform: environment.platform,
    architecture: environment.architecture,
    checks,
  }
}

export function hasDiagnosticFailures(report) {
  return report.checks.some((item) => item.status === 'fail')
}

function runCommand(command, args = [], options = {}) {
  const result = spawnSync(command, args, {
    encoding: 'utf8',
    windowsHide: true,
    shell: options.shell ?? false,
  })
  return {
    status: result.status ?? 1,
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? result.error?.message ?? '',
  }
}

export function inspectHostEnvironment(options = {}) {
  const profile = options.profile ?? 'desktop'
  const where = process.platform === 'win32'
    ? (name) => runCommand('where.exe', [name])
    : () => ({ status: 1, stdout: '', stderr: 'not applicable' })
  const environment = {
    platform: process.platform,
    architecture: process.arch,
    commands: {
      node: runCommand(process.execPath, ['--version']),
      pnpm: runCommand('pnpm', ['--version'], { shell: process.platform === 'win32' }),
      rustc: runCommand('rustc', ['-vV']),
      cargo: runCommand('cargo', ['--version']),
      linker: where('link.exe'),
      compiler: where('cl.exe'),
    },
  }
  return diagnoseEnvironment(environment, options)
}
