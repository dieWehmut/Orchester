import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { validateNativeHeader } from './binary.mjs';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');
const metaPackagePath = path.join(repositoryRoot, 'npm/cli/package.json');
const targetManifestPath = path.join(repositoryRoot, 'npm/cli/targets.json');
const repository = 'https://github.com/dieWehmut/Orchester';
const supportedIdentities = new Set([
  'linux/x64',
  'linux/arm64',
  'darwin/x64',
  'darwin/arm64',
  'win32/x64',
  'win32/arm64',
]);
const expectedRustTargets = new Map([
  ['linux/x64', 'x86_64-unknown-linux-musl'],
  ['linux/arm64', 'aarch64-unknown-linux-musl'],
  ['darwin/x64', 'x86_64-apple-darwin'],
  ['darwin/arm64', 'aarch64-apple-darwin'],
  ['win32/x64', 'x86_64-pc-windows-msvc'],
  ['win32/arm64', 'aarch64-pc-windows-msvc'],
]);

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function assertInside(root, candidate, label) {
  const relative = path.relative(root, candidate);
  if (relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) {
    return;
  }
  fail('ORCHESTER_NPM_PATH_ESCAPE', `${label} escapes its staging root`);
}

function assertSafeRelativePath(value, label) {
  if (typeof value !== 'string' || value.length === 0 || path.posix.isAbsolute(value)) {
    fail('ORCHESTER_NPM_INVALID_TARGET', `${label} must be a non-empty relative path`);
  }
  if (value.includes('\\') || value.split('/').some((part) => part === '' || part === '.' || part === '..')) {
    fail('ORCHESTER_NPM_PATH_ESCAPE', `${label} contains an unsafe path component`);
  }
}

function assertSafeSegment(value, label) {
  assertSafeRelativePath(value, label);
  if (value.includes('/')) {
    fail('ORCHESTER_NPM_PATH_ESCAPE', `${label} must contain one path segment`);
  }
}

function validateTarget(target) {
  if (!target || typeof target !== 'object' || Array.isArray(target)) {
    fail('ORCHESTER_NPM_INVALID_TARGET', 'target entries must be objects');
  }
  for (const field of ['platform', 'arch', 'packageName', 'rustTarget', 'binaryPath']) {
    if (typeof target[field] !== 'string' || target[field].length === 0) {
      fail('ORCHESTER_NPM_INVALID_TARGET', `target ${field} must be a non-empty string`);
    }
  }

  assertSafeSegment(target.platform, 'target platform');
  assertSafeSegment(target.arch, 'target architecture');
  assertSafeSegment(target.rustTarget, 'target Rust triple');
  assertSafeRelativePath(target.binaryPath, 'target binary path');

  const expectedName = `@orchester/cli-${target.platform}-${target.arch}`;
  if (target.packageName !== expectedName) {
    fail('ORCHESTER_NPM_INVALID_TARGET', `target package name must be ${expectedName}`);
  }
  const expectedBinary = target.platform === 'win32' ? 'bin/orchester.exe' : 'bin/orchester';
  if (target.binaryPath !== expectedBinary) {
    fail('ORCHESTER_NPM_INVALID_TARGET', `target binary path must be ${expectedBinary}`);
  }
  const expectedRustTarget = expectedRustTargets.get(`${target.platform}/${target.arch}`);
  if (target.rustTarget !== expectedRustTarget) {
    fail('ORCHESTER_NPM_INVALID_TARGET', `target Rust triple must be ${expectedRustTarget}`);
  }
}

function validateTargets(targets) {
  if (!Array.isArray(targets) || targets.length !== 6) {
    fail('ORCHESTER_NPM_INVALID_TARGET', 'target manifest must contain exactly six entries');
  }
  const identities = new Set();
  for (const target of targets) {
    validateTarget(target);
    const identity = `${target.platform}/${target.arch}`;
    if (!supportedIdentities.has(identity)) {
      fail('ORCHESTER_NPM_INVALID_TARGET', `unsupported target ${identity}`);
    }
    if (identities.has(identity)) {
      fail('ORCHESTER_NPM_INVALID_TARGET', `duplicate target ${identity}`);
    }
    identities.add(identity);
  }
  if ([...supportedIdentities].some((identity) => !identities.has(identity))) {
    fail('ORCHESTER_NPM_INVALID_TARGET', 'target manifest does not match the supported platform matrix');
  }
}

function validateVersion(version, metaPackage, targets) {
  if (typeof version !== 'string' || version !== metaPackage.version) {
    fail('ORCHESTER_NPM_VERSION_MISMATCH', 'staging version must match the CLI package version');
  }
  for (const { packageName } of targets) {
    if (metaPackage.optionalDependencies?.[packageName] !== version) {
      fail('ORCHESTER_NPM_VERSION_MISMATCH', `optional dependency ${packageName} must match the staging version`);
    }
  }
}

function assertOutputRootAbsent(outputRoot) {
  let status;
  try {
    status = fs.lstatSync(outputRoot);
  } catch (error) {
    if (error?.code === 'ENOENT') return;
    throw error;
  }
  fail('ORCHESTER_NPM_OUTPUT_EXISTS', status.isDirectory() && !status.isSymbolicLink()
    ? 'output root must not exist'
    : 'output root path is occupied');
}

function resolveArtifactRoot(artifactRoot) {
  const resolved = path.resolve(artifactRoot);
  let status;
  try {
    status = fs.lstatSync(resolved);
  } catch (error) {
    if (error?.code === 'ENOENT') {
      fail('ORCHESTER_NPM_ARTIFACT_ROOT_MISSING', 'artifact root must be an existing directory');
    }
    throw error;
  }
  if (!status.isDirectory() || status.isSymbolicLink()) {
    fail('ORCHESTER_NPM_ARTIFACT_ROOT_INVALID', 'artifact root must be a regular directory');
  }
  return fs.realpathSync(resolved);
}

function readNativeHeader(binary) {
  const fd = fs.openSync(binary, 'r');
  try {
    const header = Buffer.alloc(4096);
    const bytesRead = fs.readSync(fd, header, 0, header.length, 0);
    return header.subarray(0, bytesRead);
  } finally {
    fs.closeSync(fd);
  }
}

function validateBinary(binary, artifactRoot, target, runtime) {
  let status;
  try {
    status = fs.lstatSync(binary);
  } catch (error) {
    if (error?.code === 'ENOENT') {
      fail('ORCHESTER_NPM_BINARY_MISSING', `native executable is missing for ${target.rustTarget}`);
    }
    throw error;
  }

  if (status.isSymbolicLink() || !status.isFile()) {
    fail('ORCHESTER_NPM_BINARY_NOT_FILE', `native executable is not a regular file for ${target.rustTarget}`);
  }
  const realBinary = fs.realpathSync(binary);
  assertInside(artifactRoot, realBinary, 'native executable');
  if (status.size === 0) {
    fail('ORCHESTER_NPM_BINARY_NOT_EXECUTABLE', `native executable is empty for ${target.rustTarget}`);
  }
  validateNativeHeader(readNativeHeader(binary), target);
  if (target.platform !== 'win32' && runtime.platform !== 'win32' && (runtime.modeOf(binary) & 0o111) === 0) {
    fail('ORCHESTER_NPM_BINARY_NOT_EXECUTABLE', `native executable lacks execute permission for ${target.rustTarget}`);
  }
}

function platformManifest(target, version, metaPackage) {
  return {
    name: target.packageName,
    version,
    description: `Orchester native executable for ${target.platform}/${target.arch}`,
    license: metaPackage.license,
    repository,
    os: [target.platform],
    cpu: [target.arch],
    files: ['bin'],
    engines: metaPackage.engines,
    publishConfig: { access: 'public' },
  };
}

const defaultRuntime = {
  platform: process.platform,
  modeOf: (file) => fs.statSync(file).mode,
  chmod: (file, mode) => fs.chmodSync(file, mode),
  copyFile: (source, destination) => fs.copyFileSync(source, destination, fs.constants.COPYFILE_EXCL),
};

function ensureOutputParent(outputRoot) {
  const parent = path.dirname(outputRoot);
  let status;
  try {
    status = fs.lstatSync(parent);
  } catch (error) {
    if (error?.code === 'ENOENT') {
      fail('ORCHESTER_NPM_OUTPUT_PARENT_MISSING', 'output parent must be an existing directory');
    }
    throw error;
  }
  if (!status.isDirectory() || status.isSymbolicLink()) {
    fail('ORCHESTER_NPM_OUTPUT_PARENT_INVALID', 'output parent must be a regular directory');
  }
}

export function stagePlatformPackages({ artifactRoot, outputRoot, version, runtime: injectedRuntime }) {
  const runtime = { ...defaultRuntime, ...injectedRuntime };
  if (runtime.platform !== 'linux' && runtime.platform !== 'darwin') {
    fail('ORCHESTER_NPM_POSIX_HOST_REQUIRED', 'native npm staging must run on a POSIX host');
  }
  const targets = readJson(targetManifestPath);
  const metaPackage = readJson(metaPackagePath);
  validateTargets(targets);
  validateVersion(version, metaPackage, targets);

  const resolvedArtifactRoot = resolveArtifactRoot(artifactRoot);
  const resolvedOutputRoot = path.resolve(outputRoot);
  assertOutputRootAbsent(resolvedOutputRoot);
  ensureOutputParent(resolvedOutputRoot);

  const entries = targets.map((target) => {
    const binaryName = path.posix.basename(target.binaryPath);
    const sourceBinary = path.resolve(
      resolvedArtifactRoot,
      target.rustTarget,
      'release',
      binaryName,
    );
    assertInside(resolvedArtifactRoot, sourceBinary, 'native executable');

    const packageDirectory = target.packageName.split('/').at(-1);
    const packageRoot = path.resolve(resolvedOutputRoot, packageDirectory);
    assertInside(resolvedOutputRoot, packageRoot, 'platform package');
    validateBinary(sourceBinary, resolvedArtifactRoot, target, runtime);
    return { packageDirectory, sourceBinary, target };
  });

  const temporaryRoot = fs.mkdtempSync(`${resolvedOutputRoot}.tmp-`);
  let committed = false;
  try {
    for (const { packageDirectory, sourceBinary, target } of entries) {
      const packageRoot = path.join(temporaryRoot, packageDirectory);
      fs.mkdirSync(path.join(packageRoot, 'bin'), { recursive: true });
      fs.writeFileSync(
        path.join(packageRoot, 'package.json'),
        `${JSON.stringify(platformManifest(target, version, metaPackage), null, 2)}\n`,
        { flag: 'wx' },
      );
      const destinationBinary = path.join(packageRoot, target.binaryPath);
      runtime.copyFile(sourceBinary, destinationBinary);
      runtime.chmod(destinationBinary, 0o755);
      if (target.platform !== 'win32' && (runtime.modeOf(destinationBinary) & 0o111) === 0) {
        fail('ORCHESTER_NPM_BINARY_NOT_EXECUTABLE', `staged executable lacks execute permission for ${target.rustTarget}`);
      }
    }
    fs.renameSync(temporaryRoot, resolvedOutputRoot);
    committed = true;
  } finally {
    if (!committed) fs.rmSync(temporaryRoot, { force: true, recursive: true });
  }

  return entries.map(({ packageDirectory, target }) => ({
    packageName: target.packageName,
    packageRoot: path.join(resolvedOutputRoot, packageDirectory),
  }));
}

function parseCommandLine(args) {
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const option = args[index];
    const value = args[index + 1];
    if (!['--artifacts', '--output', '--version'].includes(option) || value === undefined || values.has(option)) {
      fail(
        'ORCHESTER_NPM_USAGE',
        'usage: node werkzeug/npm/stage.mjs --artifacts <dir> --output <dir> --version <version>',
      );
    }
    values.set(option, value);
  }
  if (values.size !== 3) {
    fail(
      'ORCHESTER_NPM_USAGE',
      'usage: node werkzeug/npm/stage.mjs --artifacts <dir> --output <dir> --version <version>',
    );
  }
  return {
    artifactRoot: values.get('--artifacts'),
    outputRoot: values.get('--output'),
    version: values.get('--version'),
  };
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (isMain) {
  try {
    const options = parseCommandLine(process.argv.slice(2));
    const staged = stagePlatformPackages(options);
    process.stdout.write(`Staged ${staged.length} native npm packages in ${path.resolve(options.outputRoot)}\n`);
  } catch (error) {
    const message = typeof error?.code === 'string' && error.code.startsWith('ORCHESTER_NPM_')
      ? error.message
      : 'native npm package staging failed';
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  }
}
