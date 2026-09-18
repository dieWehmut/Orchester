import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');
const desktopRoot = path.join(repositoryRoot, 'apps/desktop');
const webDist = path.join(repositoryRoot, 'apps/web/dist');
const outputDirectory = path.join(repositoryRoot, 'desktop-installers');

const architectures = new Map([
  ['x64', 'x86_64-pc-windows-msvc'],
  ['arm64', 'aarch64-pc-windows-msvc'],
]);

export function fail(message) {
  const error = new Error(`build-installer: ${message}`);
  error.code = 'ORCHESTER_DESKTOP_BUILD';
  throw error;
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const arch = options.arch;
  const rustTarget = options['rust-target'];
  const version = options.version;
  if (!arch || !rustTarget || !version) fail('--arch, --rust-target, and --version are required');
  if (architectures.get(arch) !== rustTarget) fail(`--rust-target must be ${architectures.get(arch)} for ${arch}`);

  // The WebUI payload is what this script produces for the bundle, so it is
  // built before the inputs are asserted. Asserting it first meant a clean
  // checkout always failed here: CI has no apps/web/dist, and the release job
  // died before it ever reached the build.
  run('pnpm', ['--filter', '@orchester/web', 'build']);
  assertInputs(version);

  run('pnpm', ['--filter', '@orchester/desktop', 'exec', 'tauri', 'build', '--target', rustTarget, '--bundles', 'nsis']);

  const bundleDirectory = path.join(desktopRoot, 'src-tauri/target', rustTarget, 'release/bundle/nsis');
  const produced = fs.existsSync(bundleDirectory)
    ? fs.readdirSync(bundleDirectory).filter((name) => name.endsWith('.exe'))
    : [];
  if (produced.length !== 1) fail(`expected one NSIS installer in ${bundleDirectory}, found ${produced.length}`);

  fs.mkdirSync(outputDirectory, { recursive: true });
  const destination = path.join(outputDirectory, installerName(version, arch));
  fs.copyFileSync(path.join(bundleDirectory, produced[0]), destination);
  const size = fs.statSync(destination).size;
  if (size <= 0) fail('produced installer is empty');
  process.stdout.write(`build-installer: wrote ${path.relative(repositoryRoot, destination)} (${size} bytes)\n`);
}

export function parseArguments(args) {
  const options = {};
  for (let index = 0; index < args.length; index += 1) {
    const flag = args[index];
    if (!flag.startsWith('--')) fail(`unexpected argument ${flag}`);
    const value = args[index + 1];
    if (value === undefined || value.startsWith('--')) fail(`${flag} requires a value`);
    options[flag.slice(2)] = value;
    index += 1;
  }
  return options;
}

export function installerName(version, arch) {
  if (!architectures.has(arch)) fail(`arch must be one of ${[...architectures.keys()].join(', ')}`);
  if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$/.test(version)) {
    fail('version must be pinned semver');
  }
  return `Orchester_${version}_${arch}-setup.exe`;
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? repositoryRoot,
    stdio: 'inherit',
    // pnpm is a .cmd shim on Windows and spawnSync without a shell cannot
    // resolve it, so the installer jobs died with "pnpm failed to start:
    // spawnSync pnpm ENOENT" while the same name resolves on Unix. Every
    // argument here is static or validated (semver version, known arch), so the
    // shell buys resolution without adding an injection surface.
    shell: process.platform === 'win32',
    env: process.env,
  });
  if (result.error) fail(`${command} failed to start: ${result.error.message}`);
  if (result.status !== 0) fail(`${command} ${args.join(' ')} exited with ${result.status}`);
}

export function assertInputs(version) {
  const configPath = path.join(desktopRoot, 'src-tauri/tauri.conf.json');
  const config = JSON.parse(fs.readFileSync(configPath, 'utf8'));
  if (config.version !== version) fail(`tauri.conf.json version ${config.version} does not match ${version}`);
  const manifest = JSON.parse(fs.readFileSync(path.join(desktopRoot, 'package.json'), 'utf8'));
  if (manifest.version !== version) fail(`desktop package version ${manifest.version} does not match ${version}`);
  const cargo = fs.readFileSync(path.join(desktopRoot, 'src-tauri/Cargo.toml'), 'utf8');
  const cargoVersion = /^version = "(.+)"$/m.exec(cargo)?.[1];
  if (cargoVersion !== version) fail(`desktop crate version ${cargoVersion} does not match ${version}`);
  if (!fs.existsSync(path.join(webDist, 'index.html'))) {
    fail('apps/web/dist/index.html is missing; build the WebUI payload first');
  }
}

const invokedDirectly = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`${error.message}\n`);
    process.exit(1);
  }
}