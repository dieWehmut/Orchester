import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const MAX_JSON_BYTES = 64 * 1024;
const MAX_MANIFEST_BYTES = 1024 * 1024;
const PACKAGE_MEMBERS = new Set(['manifests', 'orchester-plugin.json', 'package.json']);
const PACKAGE_FIELDS = new Set([
  'description',
  'files',
  'license',
  'name',
  'publishConfig',
  'repository',
  'version',
]);
const DESCRIPTOR_FIELDS = new Set([
  'adapterManifest',
  'command',
  'displayName',
  'name',
  'packageName',
  'schemaVersion',
  'version',
]);
const EXECUTABLE_PACKAGE_FIELDS = [
  'bin',
  'bundledDependencies',
  'dependencies',
  'devDependencies',
  'optionalDependencies',
  'peerDependencies',
  'scripts',
];
const SAFE_NAME = /^[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?$/;
const SAFE_COMMAND = /^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/;
const SAFE_VERSION = /^(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?$/;
const USAGE = 'usage: node werkzeug/npm/plugin.mjs --package <dir> --canonical <file> --name <package> --version <version>';

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function isRecord(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function exactKeys(value, expected) {
  const keys = Object.keys(value);
  return keys.length === expected.size && keys.every((key) => expected.has(key));
}

function assertInside(root, candidate) {
  const relative = path.relative(root, candidate);
  if (relative !== '' && (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative))) {
    fail('ORCHESTER_PLUGIN_PATH_ESCAPE', 'plugin member escapes its package root');
  }
}

function resolveDirectory(input) {
  const resolved = path.resolve(input);
  let status;
  try {
    status = fs.lstatSync(resolved);
  } catch {
    fail('ORCHESTER_PLUGIN_PACKAGE_DIRECTORY', 'plugin package directory is unavailable');
  }
  if (status.isSymbolicLink() || !status.isDirectory()) {
    fail('ORCHESTER_PLUGIN_PACKAGE_DIRECTORY', 'plugin package root must be a regular directory');
  }
  return fs.realpathSync(resolved);
}

function safeRelativePath(value) {
  return typeof value === 'string'
    && value.length > 0
    && !path.posix.isAbsolute(value)
    && !value.includes('\\')
    && value.split('/').every((part) => part !== '' && part !== '.' && part !== '..');
}

function readRegularFile(root, relative, maximum, code) {
  if (!safeRelativePath(relative)) fail('ORCHESTER_PLUGIN_PATH_ESCAPE', 'plugin member path is invalid');
  const candidate = path.resolve(root, ...relative.split('/'));
  assertInside(root, candidate);
  let status;
  try {
    status = fs.lstatSync(candidate);
  } catch {
    fail(code, 'required plugin package member is unavailable');
  }
  if (status.isSymbolicLink() || !status.isFile() || status.size > maximum) {
    fail(code, 'plugin package member has an invalid type or size');
  }
  assertInside(root, fs.realpathSync(candidate));
  return fs.readFileSync(candidate, 'utf8');
}

function readJson(root, relative) {
  const source = readRegularFile(root, relative, MAX_JSON_BYTES, 'ORCHESTER_PLUGIN_JSON_INVALID');
  try {
    const value = JSON.parse(source);
    if (!isRecord(value)) throw new Error('not an object');
    return value;
  } catch {
    fail('ORCHESTER_PLUGIN_JSON_INVALID', 'plugin JSON is invalid');
  }
}

function validateMembers(root, adapterManifest) {
  const entries = fs.readdirSync(root, { withFileTypes: true });
  if (entries.length !== PACKAGE_MEMBERS.size
    || entries.some((entry) => !PACKAGE_MEMBERS.has(entry.name) || entry.isSymbolicLink())) {
    fail('ORCHESTER_PLUGIN_PACKAGE_MEMBERS', 'plugin package has undeclared members');
  }
  const manifestDirectory = path.join(root, 'manifests');
  const status = fs.lstatSync(manifestDirectory);
  if (!status.isDirectory() || status.isSymbolicLink()) {
    fail('ORCHESTER_PLUGIN_PACKAGE_MEMBERS', 'plugin manifest directory is invalid');
  }
  const manifestName = path.posix.basename(adapterManifest);
  const manifests = fs.readdirSync(manifestDirectory, { withFileTypes: true });
  if (manifests.length !== 1
    || manifests[0].name !== manifestName
    || !manifests[0].isFile()
    || manifests[0].isSymbolicLink()) {
    fail('ORCHESTER_PLUGIN_PACKAGE_MEMBERS', 'plugin manifest members are invalid');
  }
}

function validatePackageManifest(manifest, expectedName, expectedVersion) {
  if (EXECUTABLE_PACKAGE_FIELDS.some((field) => Object.hasOwn(manifest, field))) {
    fail('ORCHESTER_PLUGIN_EXECUTABLE_PACKAGE', 'agent plugin packages must contain data only');
  }
  if (!exactKeys(manifest, PACKAGE_FIELDS)
    || manifest.name !== expectedName
    || manifest.version !== expectedVersion
    || manifest.description !== 'Pure-data agent adapter plugin for Orchester'
    || manifest.license !== 'MIT OR Apache-2.0'
    || manifest.repository !== 'https://github.com/dieWehmut/Orchester'
    || !Array.isArray(manifest.files)
    || manifest.files.length !== 2
    || manifest.files[0] !== 'orchester-plugin.json'
    || manifest.files[1] !== 'manifests'
    || !isRecord(manifest.publishConfig)
    || !exactKeys(manifest.publishConfig, new Set(['access']))
    || manifest.publishConfig.access !== 'public') {
    fail('ORCHESTER_PLUGIN_PACKAGE_INVALID', 'plugin package manifest does not match the locked contract');
  }
}

function validateDescriptor(descriptor, expectedName, expectedVersion) {
  const name = expectedName.startsWith('@orchester/') ? expectedName.slice('@orchester/'.length) : '';
  const expectedAdapter = `manifests/${name}.toml`;
  if (!exactKeys(descriptor, DESCRIPTOR_FIELDS)
    || descriptor.schemaVersion !== 1
    || !SAFE_NAME.test(name)
    || descriptor.name !== name
    || descriptor.packageName !== expectedName
    || descriptor.version !== expectedVersion
    || !SAFE_VERSION.test(descriptor.version)
    || typeof descriptor.displayName !== 'string'
    || descriptor.displayName.length === 0
    || descriptor.displayName.length > 64
    || [...descriptor.displayName].some((character) => /\p{Cc}|\p{Cf}/u.test(character))
    || descriptor.adapterManifest !== expectedAdapter
    || !safeRelativePath(descriptor.adapterManifest)
    || !SAFE_COMMAND.test(descriptor.command)) {
    fail('ORCHESTER_PLUGIN_DESCRIPTOR_INVALID', 'plugin descriptor does not match the locked contract');
  }
  return descriptor;
}

function normalizeToml(source) {
  let output = '';
  let quote = '';
  let escaped = false;
  for (let index = 0; index < source.length; index += 1) {
    const character = source[index];
    if (quote) {
      output += character;
      if (quote === '"' && character === '\\' && !escaped) {
        escaped = true;
      } else {
        if (character === quote && !escaped) quote = '';
        escaped = false;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      output += character;
      continue;
    }
    if (character === '#') {
      while (index + 1 < source.length && !['\r', '\n'].includes(source[index + 1])) index += 1;
      continue;
    }
    if (/\s/u.test(character)) continue;
    output += character;
  }
  if (quote) fail('ORCHESTER_PLUGIN_MANIFEST_DRIFT', 'plugin manifest contains an unterminated string');
  return output;
}

export function verifyAgentPluginPackage({
  canonicalManifest,
  expectedName,
  expectedVersion,
  packageRoot,
}) {
  if (typeof expectedName !== 'string'
    || typeof expectedVersion !== 'string'
    || !SAFE_VERSION.test(expectedVersion)) {
    fail('ORCHESTER_PLUGIN_EXPECTATION_INVALID', 'plugin verification expectations are invalid');
  }
  const root = resolveDirectory(packageRoot);
  const packageManifest = readJson(root, 'package.json');
  validatePackageManifest(packageManifest, expectedName, expectedVersion);
  const descriptor = validateDescriptor(readJson(root, 'orchester-plugin.json'), expectedName, expectedVersion);
  validateMembers(root, descriptor.adapterManifest);
  const packagedManifest = readRegularFile(
    root,
    descriptor.adapterManifest,
    MAX_MANIFEST_BYTES,
    'ORCHESTER_PLUGIN_PACKAGE_MEMBERS',
  );
  const canonical = fs.readFileSync(canonicalManifest, 'utf8');
  if (Buffer.byteLength(canonical) > MAX_MANIFEST_BYTES
    || normalizeToml(packagedManifest) !== normalizeToml(canonical)) {
    fail('ORCHESTER_PLUGIN_MANIFEST_DRIFT', 'packaged adapter manifest differs from its canonical source');
  }
  return {
    adapterManifest: descriptor.adapterManifest,
    command: descriptor.command,
    name: descriptor.name,
    packageName: descriptor.packageName,
    version: descriptor.version,
  };
}

function parseCommandLine(args) {
  const allowed = new Set(['--canonical', '--name', '--package', '--version']);
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const option = args[index];
    const value = args[index + 1];
    if (!allowed.has(option) || value === undefined || values.has(option)) {
      fail('ORCHESTER_PLUGIN_USAGE', USAGE);
    }
    values.set(option, value);
  }
  if (values.size !== allowed.size) fail('ORCHESTER_PLUGIN_USAGE', USAGE);
  return {
    canonicalManifest: values.get('--canonical'),
    expectedName: values.get('--name'),
    expectedVersion: values.get('--version'),
    packageRoot: values.get('--package'),
  };
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (isMain) {
  try {
    const verified = verifyAgentPluginPackage(parseCommandLine(process.argv.slice(2)));
    process.stdout.write(`Verified ${verified.packageName}@${verified.version}\n`);
  } catch (error) {
    const message = typeof error?.code === 'string' && error.code.startsWith('ORCHESTER_PLUGIN_')
      ? error.message
      : 'agent plugin package verification failed';
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  }
}
