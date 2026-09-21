import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');

const expectedArchitectures = ['x64', 'arm64'];

export function fail(message) {
  const error = new Error(`stage-release: ${message}`);
  error.code = 'ORCHESTER_DESKTOP_STAGE';
  throw error;
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

export function expectedAssets(version) {
  if (!/^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$/.test(version)) {
    fail('version must be pinned semver');
  }
  return expectedArchitectures.map((arch) => `Orchester_${version}_${arch}-setup.exe`);
}

export function sha256(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const installers = options.installers;
  const output = options.output;
  const version = options.version;
  if (!installers || !output || !version) fail('--installers, --output, and --version are required');

  const assets = expectedAssets(version);
  const present = fs.existsSync(installers)
    ? fs.readdirSync(installers).filter((name) => name.endsWith('.exe')).sort()
    : [];
  const expectedNames = [...assets].sort();
  if (JSON.stringify(present) !== JSON.stringify(expectedNames)) {
    fail(`expected installers ${expectedNames.join(', ')}, found ${present.join(', ') || 'none'}`);
  }

  fs.rmSync(output, { recursive: true, force: true });
  fs.mkdirSync(output, { recursive: true });
  for (const asset of assets) {
    const source = path.join(installers, asset);
    if (fs.statSync(source).size <= 0) fail(`${asset} is empty`);
    fs.copyFileSync(source, path.join(output, asset));
  }

  const sums = assets.map((asset) => `${sha256(path.join(output, asset))}  ${asset}`).join('\n') + '\n';
  fs.writeFileSync(path.join(output, 'SHA256SUMS'), sums, 'utf8');

  const archive = path.join(repositoryRoot, 'desktop-release.tar.gz');
  fs.rmSync(archive, { force: true });
  const packed = spawnSync('tar', ['-C', output, '-czf', archive, ...assets, 'SHA256SUMS'], {
    stdio: 'inherit',
    shell: false,
  });
  if (packed.error) fail(`tar failed to start: ${packed.error.message}`);
  if (packed.status !== 0) fail(`tar exited with ${packed.status}`);
  process.stdout.write(`stage-release: staged ${assets.join(', ')} and SHA256SUMS\n`);
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
