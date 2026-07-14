import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';

const TAR_BLOCK_SIZE = 512;
const MAX_PACKED_BYTES = 256 * 1024 * 1024;
const MAX_UNPACKED_BYTES = 512 * 1024 * 1024;
const MAX_TAR_MEMBERS = 64;
const META_NAME = '@orchester/cli';
const PLATFORM_NAME = /^@orchester\/cli-(linux|darwin|win32)-(x64|arm64)$/;
const USAGE = 'usage: node werkzeug/npm/verify.mjs --meta <dir> --platform-dir <dir> [--platform-dir <dir> ...]';

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function readManifest(packageRoot) {
  const file = path.join(packageRoot, 'package.json');
  let contents;
  try {
    contents = fs.readFileSync(file, 'utf8');
  } catch (error) {
    if (error?.code === 'ENOENT') {
      fail('ORCHESTER_NPM_PACKAGE_MANIFEST', `package manifest is missing in ${packageRoot}`);
    }
    throw error;
  }

  let manifest;
  try {
    manifest = JSON.parse(contents);
  } catch {
    fail('ORCHESTER_NPM_PACKAGE_MANIFEST', `package manifest is not valid JSON in ${packageRoot}`);
  }
  if (!isRecord(manifest) || typeof manifest.name !== 'string' || manifest.name.length === 0
    || typeof manifest.version !== 'string' || manifest.version.length === 0) {
    fail('ORCHESTER_NPM_PACKAGE_MANIFEST', `package manifest has no valid name and version in ${packageRoot}`);
  }
  return manifest;
}

function resolvePackageRoot(input, label) {
  if (typeof input !== 'string' || input.length === 0) {
    fail('ORCHESTER_NPM_PACKAGE_DIRECTORY', `${label} must be a non-empty directory path`);
  }
  const resolved = path.resolve(input);
  let status;
  try {
    status = fs.lstatSync(resolved);
  } catch (error) {
    if (error?.code === 'ENOENT') {
      fail('ORCHESTER_NPM_PACKAGE_DIRECTORY', `${label} does not exist`);
    }
    throw error;
  }
  if (status.isSymbolicLink() || !status.isDirectory()) {
    fail('ORCHESTER_NPM_PACKAGE_DIRECTORY', `${label} must be a regular directory`);
  }
  return fs.realpathSync(resolved);
}

function decodeTarString(header, offset, length, label) {
  const field = header.subarray(offset, offset + length);
  const nul = field.indexOf(0);
  const bytes = nul === -1 ? field : field.subarray(0, nul);
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  } catch {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', `tar ${label} is not valid UTF-8`);
  }
}

function parseTarNumber(header, offset, length, label) {
  const bytes = header.subarray(offset, offset + length);
  if ((bytes[0] & 0x80) !== 0) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', `tar ${label} uses an unsupported numeric encoding`);
  }
  const raw = Buffer.from(bytes).toString('ascii').replace(/\0.*$/s, '').trim();
  if (raw === '') return 0;
  if (!/^[0-7]+$/.test(raw)) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', `tar ${label} is not an octal number`);
  }
  const value = Number.parseInt(raw, 8);
  if (!Number.isSafeInteger(value)) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', `tar ${label} exceeds the safe integer range`);
  }
  return value;
}

function verifyTarChecksum(header) {
  const expected = parseTarNumber(header, 148, 8, 'checksum');
  let actual = 0;
  for (let index = 0; index < TAR_BLOCK_SIZE; index += 1) {
    actual += index >= 148 && index < 156 ? 0x20 : header[index];
  }
  if (actual !== expected) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar header checksum does not match its contents');
  }
}

function parsePaxRecords(payload) {
  const records = new Map();
  let offset = 0;
  while (offset < payload.length) {
    const space = payload.indexOf(0x20, offset);
    if (space === -1) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'PAX record has no length delimiter');
    }
    const lengthText = payload.subarray(offset, space).toString('ascii');
    if (!/^[1-9][0-9]*$/.test(lengthText)) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'PAX record length is invalid');
    }
    const length = Number.parseInt(lengthText, 10);
    const end = offset + length;
    if (!Number.isSafeInteger(length) || end > payload.length || payload[end - 1] !== 0x0a) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'PAX record exceeds its header body');
    }
    const record = payload.subarray(space + 1, end - 1);
    const equals = record.indexOf(0x3d);
    if (equals <= 0) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'PAX record has no key/value delimiter');
    }
    let key;
    let value;
    try {
      const decoder = new TextDecoder('utf-8', { fatal: true });
      key = decoder.decode(record.subarray(0, equals));
      value = decoder.decode(record.subarray(equals + 1));
    } catch {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'PAX record is not valid UTF-8');
    }
    if (records.has(key)) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', `PAX record repeats ${key}`);
    }
    records.set(key, value);
    offset = end;
  }
  return records;
}

function memberPath(raw) {
  if (!raw.startsWith('package/')) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar member is outside the npm package prefix');
  }
  const relative = raw.slice('package/'.length);
  if (relative.length === 0 || relative.includes('\\') || relative.includes('\0')
    || path.posix.isAbsolute(relative)
    || relative.split('/').some((part) => part === '' || part === '.' || part === '..')) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', `tar member has an unsafe path: ${raw}`);
  }
  return relative;
}

export function parseTarArchive(archive) {
  const entries = [];
  const seen = new Set();
  let offset = 0;
  let pendingPax = null;
  let zeroBlocks = 0;
  let headerCount = 0;

  while (offset + TAR_BLOCK_SIZE <= archive.length) {
    const header = archive.subarray(offset, offset + TAR_BLOCK_SIZE);
    offset += TAR_BLOCK_SIZE;
    if (header.every((byte) => byte === 0)) {
      zeroBlocks += 1;
      if (zeroBlocks === 2) break;
      continue;
    }
    if (zeroBlocks !== 0) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar contains data after an end marker');
    }

    headerCount += 1;
    if (headerCount > MAX_TAR_MEMBERS) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar archive contains too many members');
    }
    verifyTarChecksum(header);
    const magic = decodeTarString(header, 257, 6, 'magic');
    const version = decodeTarString(header, 263, 2, 'version');
    if (magic !== 'ustar' || version !== '00') {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar member does not use the ustar format');
    }
    const uid = parseTarNumber(header, 108, 8, 'uid');
    const gid = parseTarNumber(header, 116, 8, 'gid');
    if (uid !== 0 || gid !== 0) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar member owner must be uid 0 and gid 0');
    }
    const size = parseTarNumber(header, 124, 12, 'size');
    const paddedSize = Math.ceil(size / TAR_BLOCK_SIZE) * TAR_BLOCK_SIZE;
    if (offset + paddedSize > archive.length) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar member body exceeds the archive');
    }
    const body = archive.subarray(offset, offset + size);
    offset += paddedSize;

    const type = header[156] === 0 ? '0' : String.fromCharCode(header[156]);
    if (type === 'x') {
      if (pendingPax !== null) {
        fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar contains consecutive local PAX headers');
      }
      pendingPax = parsePaxRecords(body);
      continue;
    }
    if (type === '1' || type === '2') {
      fail('ORCHESTER_NPM_PACKAGE_SYMLINK', 'npm package archive contains a link member');
    }
    if (type !== '0') {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', `npm package archive contains unsupported tar member type ${type}`);
    }

    const prefix = decodeTarString(header, 345, 155, 'prefix');
    const name = decodeTarString(header, 0, 100, 'name');
    const headerPath = prefix.length === 0 ? name : `${prefix}/${name}`;
    const paxPath = pendingPax?.get('path');
    const paxSize = pendingPax?.get('size');
    if (paxSize !== undefined && (!/^(?:0|[1-9][0-9]*)$/.test(paxSize) || Number(paxSize) !== size)) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'PAX size does not match its tar header');
    }
    for (const key of pendingPax?.keys() ?? []) {
      if (!['path', 'size', 'mtime', 'atime', 'ctime'].includes(key)) {
        fail('ORCHESTER_NPM_ARCHIVE_INVALID', `unsupported PAX attribute ${key}`);
      }
    }
    pendingPax = null;

    const relative = memberPath(paxPath ?? headerPath);
    const mode = parseTarNumber(header, 100, 8, 'mode');
    if (mode > 0o7777) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar member mode contains unsupported file type bits');
    }
    if (seen.has(relative)) {
      fail('ORCHESTER_NPM_ARCHIVE_INVALID', `tar member is duplicated: ${relative}`);
    }
    seen.add(relative);
    entries.push({
      body: Buffer.from(body),
      mode,
      path: relative,
    });
  }

  if (pendingPax !== null || zeroBlocks < 2) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar archive has no complete end marker');
  }
  if (archive.subarray(offset).some((byte) => byte !== 0)) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'tar archive contains non-zero trailing data');
  }
  return entries;
}

function unpackTarball(tarball) {
  const status = fs.lstatSync(tarball);
  if (status.isSymbolicLink() || !status.isFile() || status.size > MAX_PACKED_BYTES) {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'npm pack did not create a bounded regular tarball');
  }
  let archive;
  try {
    archive = zlib.gunzipSync(fs.readFileSync(tarball), { maxOutputLength: MAX_UNPACKED_BYTES });
  } catch {
    fail('ORCHESTER_NPM_ARCHIVE_INVALID', 'npm pack tarball is not a bounded gzip archive');
  }
  return parseTarArchive(archive);
}

function parsePackOutput(stdout, manifest, destination) {
  let output;
  try {
    output = JSON.parse(stdout);
  } catch {
    fail('ORCHESTER_NPM_PACK_OUTPUT', 'npm pack did not return valid JSON');
  }
  if (!Array.isArray(output) || output.length !== 1 || !isRecord(output[0])) {
    fail('ORCHESTER_NPM_PACK_OUTPUT', 'npm pack returned an unexpected result count');
  }
  const result = output[0];
  if (result.name !== manifest.name) {
    fail('ORCHESTER_NPM_PACKAGE_NAME_MISMATCH', 'npm pack package name does not match package.json');
  }
  if (result.version !== manifest.version) {
    fail('ORCHESTER_NPM_PACKAGE_VERSION_MISMATCH', 'npm pack package version does not match package.json');
  }
  if (typeof result.filename !== 'string' || path.basename(result.filename) !== result.filename) {
    fail('ORCHESTER_NPM_PACK_OUTPUT', 'npm pack returned an unsafe tarball name');
  }
  if (!Array.isArray(result.files)) {
    fail('ORCHESTER_NPM_PACK_OUTPUT', 'npm pack returned no file manifest');
  }
  const tarball = path.join(destination, result.filename);
  const relative = path.relative(destination, tarball);
  if (relative.startsWith(`..${path.sep}`) || relative === '..' || path.isAbsolute(relative)) {
    fail('ORCHESTER_NPM_PACK_OUTPUT', 'npm pack tarball escapes its destination');
  }
  return { result, tarball };
}

function runPack(packageRoot, destination) {
  const packed = spawnSync('npm', [
    'pack',
    '--json',
    '--ignore-scripts',
    '--offline',
    '--pack-destination',
    destination,
  ], {
    cwd: packageRoot,
    encoding: 'utf8',
    env: {
      ...process.env,
      npm_config_audit: 'false',
      npm_config_fund: 'false',
      npm_config_ignore_scripts: 'true',
      npm_config_offline: 'true',
      npm_config_update_notifier: 'false',
    },
    maxBuffer: 4 * 1024 * 1024,
    shell: false,
    timeout: 120_000,
    windowsHide: true,
  });
  if (packed.error || packed.status !== 0) {
    fail('ORCHESTER_NPM_PACK_FAILED', `npm pack failed for ${path.basename(packageRoot)}`);
  }
  return packed.stdout;
}

function metadataPaths(files) {
  const paths = [];
  const seen = new Set();
  for (const file of files) {
    if (!isRecord(file) || typeof file.path !== 'string' || file.path.length === 0
      || file.path.includes('\\') || file.path.includes('\0') || path.posix.isAbsolute(file.path)
      || file.path.split('/').some((part) => part === '' || part === '.' || part === '..')) {
      fail('ORCHESTER_NPM_PACK_OUTPUT', 'npm pack returned an unsafe file path');
    }
    if (seen.has(file.path)) {
      fail('ORCHESTER_NPM_PACK_OUTPUT', `npm pack repeated file path ${file.path}`);
    }
    seen.add(file.path);
    paths.push(file.path);
  }
  return paths;
}

function assertSameMembers(metadata, entries) {
  const packed = metadataPaths(metadata.files).sort();
  const archived = entries.map((entry) => entry.path).sort();
  if (packed.length !== archived.length || packed.some((member, index) => member !== archived[index])) {
    fail('ORCHESTER_NPM_PACK_OUTPUT', 'npm pack JSON and tar members do not match');
  }
}

function assertMetaMembers(entries) {
  const required = new Set([
    'bin/orchester.cjs',
    'lib/process.cjs',
    'lib/target.cjs',
    'package.json',
    'targets.json',
  ]);
  for (const entry of entries) {
    if (!required.has(entry.path)) {
      const member = entry.path;
      fail('ORCHESTER_NPM_PACKAGE_EXTRA_MEMBER', `meta package contains unexpected member ${member}`);
    }
    if (entry.path === 'bin/orchester.cjs' && (entry.mode & 0o7777) !== 0o755) {
      fail('ORCHESTER_NPM_PACKAGE_MODE', 'meta launcher member bin/orchester.cjs must have mode 0755');
    }
    required.delete(entry.path);
  }
  if (required.size !== 0) {
    fail('ORCHESTER_NPM_PACKAGE_MISSING_MEMBER', `meta package is missing ${[...required].join(', ')}`);
  }
}

function expectedPlatformBinary(packageName) {
  const match = PLATFORM_NAME.exec(packageName);
  if (match === null) {
    fail('ORCHESTER_NPM_PACKAGE_NAME_MISMATCH', `unsupported platform package name ${packageName}`);
  }
  return match[1] === 'win32' ? 'bin/orchester.exe' : 'bin/orchester';
}

function assertPlatformMembers(entries, packageName) {
  const native = expectedPlatformBinary(packageName);
  const expected = new Set(['package.json', native]);
  for (const entry of entries) {
    if (!expected.delete(entry.path)) {
      fail('ORCHESTER_NPM_PACKAGE_EXTRA_MEMBER', `platform package contains unexpected member ${entry.path}`);
    }
    if (entry.path === native && (entry.mode & 0o7777) !== 0o755) {
      fail('ORCHESTER_NPM_PACKAGE_MODE', `native tar member ${native} must have mode 0755`);
    }
  }
  if (expected.size !== 0) {
    fail('ORCHESTER_NPM_PACKAGE_MISSING_MEMBER', `platform package is missing ${[...expected].join(', ')}`);
  }
}

function assertArchivedIdentity(entries, manifest) {
  const entry = entries.find(({ path: member }) => member === 'package.json');
  if (entry === undefined) {
    fail('ORCHESTER_NPM_PACKAGE_MISSING_MEMBER', 'npm package is missing package.json');
  }
  let archived;
  try {
    archived = JSON.parse(entry.body.toString('utf8'));
  } catch {
    fail('ORCHESTER_NPM_PACKAGE_MANIFEST', 'archived package.json is not valid JSON');
  }
  if (archived?.name !== manifest.name) {
    fail('ORCHESTER_NPM_PACKAGE_NAME_MISMATCH', 'archived package name does not match its source manifest');
  }
  if (archived?.version !== manifest.version) {
    fail('ORCHESTER_NPM_PACKAGE_VERSION_MISMATCH', 'archived package version does not match its source manifest');
  }
}

function verifyPackedDirectory(packageRoot, manifest, kind, destination) {
  fs.mkdirSync(destination);
  const stdout = runPack(packageRoot, destination);
  const { result, tarball } = parsePackOutput(stdout, manifest, destination);
  const entries = unpackTarball(tarball);
  assertSameMembers(result, entries);
  assertArchivedIdentity(entries, manifest);
  if (kind === 'meta') {
    assertMetaMembers(entries);
  } else {
    assertPlatformMembers(entries, manifest.name);
  }
  return {
    archive: result.filename,
    name: manifest.name,
    version: manifest.version,
  };
}

function loadInputs(metaInput, platformInputs) {
  const metaRoot = resolvePackageRoot(metaInput, 'meta package directory');
  const meta = readManifest(metaRoot);
  if (meta.name !== META_NAME) {
    fail('ORCHESTER_NPM_PACKAGE_NAME_MISMATCH', `meta package must be named ${META_NAME}`);
  }
  if (!isRecord(meta.optionalDependencies)) {
    fail('ORCHESTER_NPM_PACKAGE_MANIFEST', 'meta package must declare native optional dependencies');
  }

  const platforms = platformInputs.map((input, index) => {
    const root = resolvePackageRoot(input, `platform package directory ${index + 1}`);
    const manifest = readManifest(root);
    expectedPlatformBinary(manifest.name);
    if (manifest.version !== meta.version || meta.optionalDependencies[manifest.name] !== meta.version) {
      fail('ORCHESTER_NPM_PACKAGE_VERSION_MISMATCH', `platform package ${manifest.name} is not pinned to ${meta.version}`);
    }
    return { manifest, root };
  });

  const names = platforms.map(({ manifest }) => manifest.name);
  if (new Set(names).size !== names.length) {
    fail('ORCHESTER_NPM_PACKAGE_NAME_MISMATCH', 'platform package directories contain duplicate package names');
  }
  const expectedNames = Object.keys(meta.optionalDependencies).sort();
  const actualNames = [...names].sort();
  if (expectedNames.length !== actualNames.length
    || expectedNames.some((name, index) => name !== actualNames[index])) {
    fail('ORCHESTER_NPM_PACKAGE_NAME_MISMATCH', 'platform package directories do not match optionalDependencies');
  }
  return { meta: { manifest: meta, root: metaRoot }, platforms };
}

export function verifyNpmPackages({ meta, platformDirs } = {}) {
  if (process.platform !== 'linux' && process.platform !== 'darwin') {
    fail('ORCHESTER_NPM_POSIX_HOST_REQUIRED', 'npm package verification must run on a POSIX host');
  }
  if (!Array.isArray(platformDirs) || platformDirs.length === 0) {
    fail('ORCHESTER_NPM_USAGE', USAGE);
  }
  const packages = loadInputs(meta, platformDirs);
  const temporaryRoot = fs.mkdtempSync(path.join(os.tmpdir(), '.orchester-npm-verify-'));
  try {
    const verified = [verifyPackedDirectory(
      packages.meta.root,
      packages.meta.manifest,
      'meta',
      path.join(temporaryRoot, 'meta'),
    )];
    for (const [index, platform] of packages.platforms.entries()) {
      verified.push(verifyPackedDirectory(
        platform.root,
        platform.manifest,
        'platform',
        path.join(temporaryRoot, `platform-${index}`),
      ));
    }
    return verified;
  } finally {
    fs.rmSync(temporaryRoot, { force: true, recursive: true });
  }
}

export function parseCommandLine(args) {
  let meta;
  const platformDirs = [];
  for (let index = 0; index < args.length; index += 2) {
    const option = args[index];
    const value = args[index + 1];
    if (value === undefined) fail('ORCHESTER_NPM_USAGE', USAGE);
    if (option === '--meta' && meta === undefined) {
      meta = value;
    } else if (option === '--platform-dir') {
      platformDirs.push(value);
    } else {
      fail('ORCHESTER_NPM_USAGE', USAGE);
    }
  }
  if (meta === undefined || platformDirs.length === 0) {
    fail('ORCHESTER_NPM_USAGE', USAGE);
  }
  return { meta, platformDirs };
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (isMain) {
  try {
    const verified = verifyNpmPackages(parseCommandLine(process.argv.slice(2)));
    process.stdout.write(`Verified ${verified.length} npm package archives\n`);
  } catch (error) {
    const message = typeof error?.code === 'string' && error.code.startsWith('ORCHESTER_NPM_')
      ? error.message
      : 'npm package verification failed';
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  }
}
