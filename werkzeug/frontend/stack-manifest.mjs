import { readFile } from 'node:fs/promises'
import { dirname, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const moduleDirectory = dirname(fileURLToPath(import.meta.url))
export const defaultRepositoryRoot = resolve(moduleDirectory, '../..')

async function readJson(path) {
  return JSON.parse(await readFile(path, 'utf8'))
}

function expectedViteUrl(surface) {
  return `http://${surface.host}:${surface.port}/`
}

function normalizedVersion(value) {
  const match = String(value).match(/(\d+)(?:\.(\d+))?(?:\.(\d+))?/)
  if (!match) return null
  const parts = match.slice(1).map((part) => Number(part ?? 0))
  while (parts.length > 1 && parts.at(-1) === 0) parts.pop()
  return parts.join('.')
}

function requireText(contents, expected, source, errors, label = expected) {
  if (!contents.includes(expected)) {
    errors.push(`${source} does not match ${label}`)
  }
}

function validateManifestShape(manifest) {
  const errors = []
  if (manifest?.schemaVersion !== 1) {
    errors.push('apps/stack.manifest.json must use schemaVersion 1')
  }

  const expectedSurfaceNames = ['webui', 'website', 'desktop']
  const surfaceNames = Object.keys(manifest?.surfaces ?? {})
  if (surfaceNames.join(',') !== expectedSurfaceNames.join(',')) {
    errors.push(`apps/stack.manifest.json surfaces must be ${expectedSurfaceNames.join(', ')}`)
  }

  for (const name of ['webui', 'website']) {
    const surface = manifest?.surfaces?.[name]
    if (surface?.kind !== 'vite') errors.push(`${name} must be a Vite surface`)
    if (typeof surface?.package !== 'string') errors.push(`${name} must name a package`)
    if (surface?.host !== '127.0.0.1') errors.push(`${name} must bind to 127.0.0.1`)
    if (!Number.isInteger(surface?.port)) errors.push(`${name} must use an integer port`)
    if (surface && surface.url !== expectedViteUrl(surface)) {
      errors.push(`${name} URL must match its host and port`)
    }
  }

  const desktop = manifest?.surfaces?.desktop
  if (desktop?.kind !== 'tauri') errors.push('desktop must be a Tauri surface')
  if (desktop?.frontend !== 'webui') errors.push('desktop frontend must be webui')
  if (typeof desktop?.package !== 'string') errors.push('desktop must name a package')

  if (manifest?.pages?.surface !== 'website') errors.push('Pages surface must be website')
  if (!manifest?.pages?.basePath?.startsWith('/') || !manifest.pages.basePath.endsWith('/')) {
    errors.push('Pages basePath must start and end with /')
  }
  if (typeof manifest?.pages?.url !== 'string') errors.push('Pages must expose its public URL')
  if (typeof manifest?.pages?.artifactDirectory !== 'string') {
    errors.push('Pages must expose its artifact directory')
  }
  if (typeof manifest?.pages?.workflow !== 'string') errors.push('Pages must expose its workflow')

  if (typeof manifest?.toolchain?.node !== 'string') errors.push('toolchain must declare Node.js')
  if (typeof manifest?.toolchain?.pagesNode !== 'string') {
    errors.push('toolchain must declare the Pages Node.js version')
  }
  if (typeof manifest?.toolchain?.pnpm !== 'string') errors.push('toolchain must declare pnpm')
  if (typeof manifest?.toolchain?.rust !== 'string') errors.push('toolchain must declare Rust')
  if (manifest?.toolchain?.windowsAbi !== 'msvc') errors.push('Windows ABI must be msvc')

  return errors
}

export async function readStackManifest(repositoryRoot = defaultRepositoryRoot) {
  return readJson(resolve(repositoryRoot, 'apps/stack.manifest.json'))
}

export async function validateStackManifest(repositoryRoot, manifest) {
  const errors = validateManifestShape(manifest)
  if (errors.length > 0) return errors

  const webui = manifest.surfaces.webui
  const website = manifest.surfaces.website
  const desktop = manifest.surfaces.desktop
  const pages = manifest.pages

  const [appsPackage, rootCargo, webPackage, websitePackage, desktopPackage, tauriConfig, webVite, websiteVite, workflow] =
    await Promise.all([
      readJson(resolve(repositoryRoot, 'apps/package.json')),
      readFile(resolve(repositoryRoot, 'Cargo.toml'), 'utf8'),
      readJson(resolve(repositoryRoot, 'apps/web/package.json')),
      readJson(resolve(repositoryRoot, 'apps/website/package.json')),
      readJson(resolve(repositoryRoot, 'apps/desktop/package.json')),
      readJson(resolve(repositoryRoot, 'apps/desktop/src-tauri/tauri.conf.json')),
      readFile(resolve(repositoryRoot, 'apps/web/vite.config.ts'), 'utf8'),
      readFile(resolve(repositoryRoot, 'apps/website/vite.config.ts'), 'utf8'),
      readFile(resolve(repositoryRoot, pages.workflow), 'utf8'),
    ])

  if (appsPackage.packageManager !== `pnpm@${manifest.toolchain.pnpm}`) {
    errors.push('apps/package.json packageManager does not match the toolchain contract')
  }
  const cargoRustVersion = rootCargo.match(/rust-version\s*=\s*"([^"]+)"/)?.[1]
  if (normalizedVersion(cargoRustVersion) !== normalizedVersion(manifest.toolchain.rust)) {
    errors.push('Cargo.toml does not match minimum Rust version')
  }

  for (const [surfaceName, surface, packageManifest] of [
    ['webui', webui, webPackage],
    ['website', website, websitePackage],
    ['desktop', desktop, desktopPackage],
  ]) {
    if (packageManifest.name !== surface.package) {
      errors.push(`${surfaceName} package does not match its package.json name`)
    }
  }

  requireText(webVite, `host: '${webui.host}'`, 'apps/web/vite.config.ts', errors, 'WebUI host')
  requireText(webVite, `port: ${webui.port}`, 'apps/web/vite.config.ts', errors, 'WebUI port')
  requireText(
    websiteVite,
    `host: '${website.host}'`,
    'apps/website/vite.config.ts',
    errors,
    'website host',
  )
  requireText(
    websiteVite,
    `port: ${website.port}`,
    'apps/website/vite.config.ts',
    errors,
    'website port',
  )

  if (tauriConfig.build?.devUrl !== webui.url.slice(0, -1)) {
    errors.push(`Tauri devUrl does not match the ${webui.url} WebUI surface`)
  }
  const beforeDevScript = tauriConfig.build?.beforeDevCommand?.script ?? ''
  requireText(beforeDevScript, `--filter ${webui.package} dev`, 'Tauri beforeDevCommand', errors)
  requireText(beforeDevScript, '--strictPort', 'Tauri beforeDevCommand', errors)

  requireText(workflow, `BASE_PATH: ${pages.basePath}`, pages.workflow, errors, 'Pages base path')
  requireText(
    workflow,
    `NODE_VERSION: "${manifest.toolchain.pagesNode}"`,
    pages.workflow,
    errors,
    'Pages Node.js version',
  )
  requireText(
    workflow,
    `--filter ${website.package} build`,
    pages.workflow,
    errors,
    'website build command',
  )
  requireText(
    workflow,
    `path: ${pages.artifactDirectory}`,
    pages.workflow,
    errors,
    'Pages artifact directory',
  )

  const expectedPagesUrl = `https://diewehmut.github.io${pages.basePath}`
  if (pages.url !== expectedPagesUrl) {
    errors.push(`Pages URL must be ${expectedPagesUrl}`)
  }

  return errors
}

export async function verifyStackManifest(repositoryRoot = defaultRepositoryRoot) {
  const manifest = await readStackManifest(repositoryRoot)
  const errors = await validateStackManifest(repositoryRoot, manifest)
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`stack: ${error}\n`)
    return 1
  }
  process.stdout.write('stack: manifest matches WebUI, website, Tauri, and Pages configuration\n')
  return 0
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  process.exitCode = await verifyStackManifest()
}
