import { spawn } from 'node:child_process'
import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

import {
  defaultRepositoryRoot,
  readStackManifest,
  validateStackManifest,
} from './stack-manifest.mjs'
import {
  hasDiagnosticFailures,
  inspectHostEnvironment,
} from './environment.mjs'
import { formatDoctorReport } from './doctor.mjs'

const surfaceNames = ['webui', 'website', 'desktop']

export function parseLaunchArguments(args) {
  const [surface, ...flags] = args
  if (!surface) throw new Error('A surface is required: webui, website, or desktop')
  if (!surfaceNames.includes(surface)) throw new Error(`Unknown surface: ${surface}`)

  const supportedFlags = new Set(['--dry-run'])
  const unsupported = flags.find((flag) => !supportedFlags.has(flag))
  if (unsupported) throw new Error(`Unknown launch option: ${unsupported}`)
  return {
    surface,
    dryRun: flags.includes('--dry-run'),
  }
}

export function createLaunchPlan(surfaceName, manifest, repositoryRoot = defaultRepositoryRoot) {
  const surface = manifest.surfaces[surfaceName]
  if (!surface) throw new Error(`Unknown surface: ${surfaceName}`)

  const args = ['--dir', 'apps', '--filter', surface.package, 'dev']
  if (surface.kind === 'vite') args.push('--', '--strictPort')

  return {
    surface: surfaceName,
    command: 'pnpm',
    args,
    cwd: repositoryRoot,
    url: surface.kind === 'vite' ? surface.url : null,
    requiresDesktopDoctor: surface.kind === 'tauri',
  }
}

export function spawnProcess(command, args, options) {
  return new Promise((resolveExit, reject) => {
    const child = spawn(command, args, options)
    child.once('error', reject)
    child.once('exit', (code, signal) => {
      if (signal) {
        process.stderr.write(`launch: child process terminated by ${signal}\n`)
        resolveExit(1)
        return
      }
      resolveExit(code ?? 1)
    })
  })
}

export async function runLaunchPlan(plan, dependencies = {}) {
  const run = dependencies.spawnProcess ?? spawnProcess
  return run(plan.command, plan.args, {
    cwd: plan.cwd,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  })
}

export async function launch(args, dependencies = {}) {
  const repositoryRoot = dependencies.repositoryRoot ?? defaultRepositoryRoot
  const options = parseLaunchArguments(args)
  const manifest = await readStackManifest(repositoryRoot)
  const errors = await validateStackManifest(repositoryRoot, manifest)
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`launch: ${error}\n`)
    return 1
  }

  const plan = createLaunchPlan(options.surface, manifest, repositoryRoot)
  if (options.dryRun) {
    process.stdout.write(`${JSON.stringify(plan, null, 2)}\n`)
    return 0
  }

  if (plan.requiresDesktopDoctor) {
    const inspect = dependencies.inspectDesktopEnvironment ?? inspectHostEnvironment
    const write = dependencies.write ?? ((chunk) => process.stdout.write(chunk))
    const report = await inspect({
      profile: 'desktop',
      minimums: {
        node: manifest.toolchain.node.replace('>=', ''),
        pnpm: manifest.toolchain.pnpm,
        rust: manifest.toolchain.rust.replace('>=', ''),
      },
    })
    write(formatDoctorReport(report))
    if (hasDiagnosticFailures(report)) return 1
  }

  if (plan.url) process.stdout.write(`launch: ${plan.surface} -> ${plan.url}\n`)
  return runLaunchPlan(plan, dependencies)
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    process.exitCode = await launch(process.argv.slice(2))
  } catch (error) {
    process.stderr.write(`launch: ${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 2
  }
}
