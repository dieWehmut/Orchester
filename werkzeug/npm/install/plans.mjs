import path from 'node:path';
import process from 'node:process';

import { FIXTURE_NAME } from './fixture.mjs';

export const SUPPORTED_MANAGERS = ['npm', 'pnpm', 'yarn', 'bun'];

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function commonNpmArgs(sandbox) {
  return [
    '--cache', sandbox.cacheDirectory,
    '--userconfig', sandbox.userConfig,
    '--registry', sandbox.registry,
    '--no-audit',
    '--no-fund',
    '--ignore-scripts',
    '--offline',
  ];
}

/** Return manager argv before the descriptor's optional Node prefix arguments. */
export function buildManagerPlan(name, sandbox, tarball) {
  if (!SUPPORTED_MANAGERS.includes(name)) fail('ORCHESTER_NPM_MANAGER_UNSUPPORTED', `unsupported package manager: ${name}`);
  if (!sandbox?.root || !sandbox?.env) fail('ORCHESTER_NPM_SANDBOX_INVALID', 'manager plan requires an isolated environment');
  const common = {
    name,
    packageName: FIXTURE_NAME,
    tarball,
    shimRoots: [],
    packageRoots: [],
    installArgs: [],
    removeArgs: [],
  };
  if (name === 'npm') {
    common.installArgs = ['install', '-g', tarball, '--prefix', sandbox.prefix, ...commonNpmArgs(sandbox)];
    common.removeArgs = ['uninstall', '-g', FIXTURE_NAME, '--prefix', sandbox.prefix, ...commonNpmArgs(sandbox)];
    common.shimRoots = [process.platform === 'win32' ? sandbox.prefix : path.join(sandbox.prefix, 'bin')];
    common.packageRoots = [path.join(sandbox.prefix, 'node_modules', FIXTURE_NAME)];
  } else if (name === 'pnpm') {
    common.installArgs = [
      'add', '--global', tarball,
      '--global-dir', sandbox.globalDirectory,
      '--global-bin-dir', sandbox.binDirectory,
      '--store-dir', sandbox.storeDirectory,
      '--cache-dir', sandbox.cacheDirectory,
      '--registry', sandbox.registry,
      '--offline', '--ignore-scripts',
    ];
    common.removeArgs = ['remove', '--global', '--global-dir', sandbox.globalDirectory, FIXTURE_NAME];
    common.shimRoots = [sandbox.binDirectory];
    common.packageRoots = [path.join(sandbox.globalDirectory, 'node_modules', FIXTURE_NAME)];
  } else if (name === 'yarn') {
    common.installArgs = [
      'global', 'add', tarball,
      '--prefix', sandbox.prefix,
      '--global-folder', sandbox.globalDirectory,
      '--cache-folder', sandbox.cacheDirectory,
      '--registry', sandbox.registry,
      '--offline', '--ignore-scripts', '--non-interactive', '--no-progress',
    ];
    common.removeArgs = [
      'global', 'remove', FIXTURE_NAME,
      '--prefix', sandbox.prefix,
      '--global-folder', sandbox.globalDirectory,
      '--cache-folder', sandbox.cacheDirectory,
      '--registry', sandbox.registry,
      '--offline', '--ignore-scripts', '--non-interactive', '--no-progress',
    ];
    common.shimRoots = [path.join(sandbox.prefix, 'bin'), sandbox.prefix];
    common.packageRoots = [path.join(sandbox.globalDirectory, 'node_modules', FIXTURE_NAME)];
  } else {
    // Bun has no offline switch. A loopback registry makes accidental network
    // access fail closed; retaining its global manifest is required for remove.
    common.installArgs = [
      'add', '--global', tarball,
      '--ignore-scripts', '--registry', sandbox.registry,
      '--cache-dir', sandbox.cacheDirectory,
      '--cwd', sandbox.bunGlobalDirectory,
    ];
    common.removeArgs = [
      'remove', '--global', FIXTURE_NAME,
      '--registry', sandbox.registry,
      '--cache-dir', sandbox.cacheDirectory,
      '--cwd', sandbox.bunGlobalDirectory,
    ];
    common.shimRoots = [path.join(sandbox.bunInstall, 'bin')];
    common.packageRoots = [path.join(sandbox.bunGlobalDirectory, 'node_modules', FIXTURE_NAME)];
  }
  return common;
}
