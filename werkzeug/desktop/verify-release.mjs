import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');
const expectedArchitectures = ['x64', 'arm64'];
const installerPrefix = 'Orchester_';

export function fail(message) {
  const error = new Error(`verify-release: ${message}`);
  error.code = 'ORCHESTER_DESKTOP_VERIFY';
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

export function parseChecksums(contents) {
  const entries = new Map();
  for (const line of contents.split('\n')) {
    if (line.trim() === '') continue;
    const match = /^([0-9a-f]{64}) {2}(.+)$/.exec(line);
    if (!match) fail(`malformed checksum line: ${line}`);
    entries.set(match[2], match[1]);
  }
  return entries;
}

export function expectedAssets(version) {
  return [
    ...expectedArchitectures.map((arch) => `${installerPrefix}${version}_${arch}-setup.exe`),
    'SHA256SUMS',
  ];
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: 'utf8', shell: false, ...options });
  if (result.error) fail(`${command} failed to start: ${result.error.message}`);
  if (result.status !== 0) fail(`${command} ${args.join(' ')} exited with ${result.status}: ${result.stderr ?? ''}`);
  return result.stdout ?? '';
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  const tag = options.tag;
  const version = options.version;
  if (!tag || !version) fail('--tag and --version are required');
  if (tag !== `desktop-v${version}`) fail(`tag ${tag} does not match version ${version}`);

  const listing = JSON.parse(
    run('gh', [
      'release',
      'view',
      tag,
      '--repo',
      process.env.GITHUB_REPOSITORY ?? 'dieWehmut/Orchester',
      '--json',
      'tagName,isDraft,isPrerelease,assets',
      '--jq',
      '.',
    ]),
  );
  if (listing.tagName !== tag) fail(`release tag is ${listing.tagName}`);
  if (listing.isDraft || listing.isPrerelease) fail('release must be published and stable');

  const assets = expectedAssets(version);
  const byName = new Map(listing.assets.map((asset) => [asset.name, asset]));
  for (const asset of assets) {
    const entry = byName.get(asset);
    if (!entry) fail(`release is missing ${asset}`);
    if (!entry.size || entry.size <= 0) fail(`${asset} is empty`);
  }

  const download = fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-release-'));
  try {
    run('gh', ['release', 'download', tag, '--repo', process.env.GITHUB_REPOSITORY ?? 'dieWehmut/Orchester', '--dir', download]);
    const sums = parseChecksums(fs.readFileSync(path.join(download, 'SHA256SUMS'), 'utf8'));
    for (const [name, digest] of sums) {
      const file = path.join(download, name);
      if (!fs.existsSync(file)) fail(`SHA256SUMS references a missing asset: ${name}`);
      const actual = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
      if (actual !== digest) fail(`${name} hash ${actual} does not match ${digest}`);
    }
    for (const name of expectedArchitectures.map((arch) => `${installerPrefix}${version}_${arch}-setup.exe`)) {
      if (!sums.has(name)) fail(`SHA256SUMS does not cover ${name}`);
    }
  } finally {
    fs.rmSync(download, { recursive: true, force: true });
  }

  process.stdout.write(`verify-release: ${tag} assets download and match SHA256SUMS\n`);
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
