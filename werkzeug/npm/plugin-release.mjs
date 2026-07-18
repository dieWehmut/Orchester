import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { verifyAgentPluginPackage } from './plugin.mjs';

export const OFFICIAL_AGENT_PLUGINS = Object.freeze(['claude', 'codex', 'opencode']);

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const defaultRepositoryRoot = path.resolve(moduleDirectory, '../..');
const USAGE = 'usage: node werkzeug/npm/plugin-release.mjs --version <version>';

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function readJson(file, code) {
  let value;
  try {
    value = JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch {
    fail(code, 'release package manifest is unavailable or invalid');
  }
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    fail(code, 'release package manifest must be an object');
  }
  return value;
}

function exactPluginDirectories(pluginsRoot) {
  let entries;
  try {
    entries = fs.readdirSync(pluginsRoot, { withFileTypes: true });
  } catch {
    fail('ORCHESTER_PLUGIN_RELEASE_MATRIX', 'official plugin package root is unavailable');
  }
  const actual = entries.map(({ name }) => name).sort();
  if (actual.length !== OFFICIAL_AGENT_PLUGINS.length
    || actual.some((name, index) => name !== OFFICIAL_AGENT_PLUGINS[index])
    || entries.some((entry) => entry.isSymbolicLink() || !entry.isDirectory())) {
    fail('ORCHESTER_PLUGIN_RELEASE_MATRIX', 'official plugin package matrix does not match the locked release set');
  }
}

export function verifyOfficialAgentPlugins({
  expectedVersion,
  repositoryRoot = defaultRepositoryRoot,
} = {}) {
  const root = path.resolve(repositoryRoot);
  const cliManifest = readJson(
    path.join(root, 'npm/cli/package.json'),
    'ORCHESTER_PLUGIN_RELEASE_VERSION',
  );
  if (cliManifest.name !== '@orchester/cli'
    || typeof expectedVersion !== 'string'
    || expectedVersion.length === 0
    || cliManifest.version !== expectedVersion) {
    fail('ORCHESTER_PLUGIN_RELEASE_VERSION', 'official plugins must match the CLI release version');
  }

  const pluginsRoot = path.join(root, 'npm/plugins');
  exactPluginDirectories(pluginsRoot);
  return OFFICIAL_AGENT_PLUGINS.map((name) => verifyAgentPluginPackage({
    canonicalManifest: path.join(root, `manifeste/${name}.toml`),
    expectedName: `@orchester/${name}`,
    expectedVersion,
    packageRoot: path.join(pluginsRoot, name),
  }));
}

function parseCommandLine(args) {
  if (args.length !== 2 || args[0] !== '--version' || args[1].length === 0) {
    fail('ORCHESTER_PLUGIN_RELEASE_USAGE', USAGE);
  }
  return { expectedVersion: args[1] };
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (isMain) {
  try {
    const verified = verifyOfficialAgentPlugins(parseCommandLine(process.argv.slice(2)));
    process.stdout.write(`Verified ${verified.length} official agent plugin packages\n`);
  } catch (error) {
    const message = typeof error?.code === 'string' && error.code.startsWith('ORCHESTER_PLUGIN_')
      ? error.message
      : 'official agent plugin release verification failed';
    process.stderr.write(`${message}\n`);
    process.exitCode = 1;
  }
}
