import { resolve } from 'node:path'
import { pathToFileURL } from 'node:url'

import {
  hasDiagnosticFailures,
  inspectHostEnvironment,
} from './environment.mjs'
import {
  defaultRepositoryRoot,
  readStackManifest,
} from './stack-manifest.mjs'

function parseDoctorArguments(args) {
  const [profile = 'desktop', ...flags] = args
  if (!['web', 'desktop'].includes(profile)) throw new Error(`Unknown doctor profile: ${profile}`)
  const unsupported = flags.find((flag) => flag !== '--json')
  if (unsupported) throw new Error(`Unknown doctor option: ${unsupported}`)
  return { profile, json: flags.includes('--json') }
}

export function formatDoctorReport(report) {
  const lines = [`Environment: ${report.profile} on ${report.platform}/${report.architecture}`]
  for (const item of report.checks) {
    lines.push(`${item.status.toUpperCase().padEnd(4)} ${item.id}: ${item.message}`)
    if (item.remediation) lines.push(`     Fix: ${item.remediation}`)
  }
  return `${lines.join('\n')}\n`
}

export async function runDoctor(args, dependencies = {}) {
  const options = parseDoctorArguments(args)
  const inspect = dependencies.inspect ?? inspectHostEnvironment
  const write = dependencies.write ?? ((chunk) => process.stdout.write(chunk))
  const readManifest = dependencies.readManifest ?? readStackManifest
  const manifest = await readManifest(dependencies.repositoryRoot ?? defaultRepositoryRoot)
  const report = await inspect({
    profile: options.profile,
    minimums: {
      node: manifest.toolchain.node.replace('>=', ''),
      pnpm: manifest.toolchain.pnpm,
      rust: manifest.toolchain.rust.replace('>=', ''),
    },
  })
  write(options.json ? `${JSON.stringify(report, null, 2)}\n` : formatDoctorReport(report))
  return hasDiagnosticFailures(report) ? 1 : 0
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    process.exitCode = await runDoctor(process.argv.slice(2))
  } catch (error) {
    process.stderr.write(`doctor: ${error instanceof Error ? error.message : String(error)}\n`)
    process.exitCode = 2
  }
}
