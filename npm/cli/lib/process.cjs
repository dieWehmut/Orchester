'use strict';

const { spawn: spawnProcess } = require('node:child_process');
const path = require('node:path');

const packageJson = require('../package.json');
const { resolveTarget } = require('./target.cjs');

const SPAWN_ERROR_MESSAGE = 'Failed to start the Orchester native executable.';

class MissingNativePackageError extends Error {
  constructor(target) {
    const metaPackage = `${packageJson.name}@${packageJson.version}`;
    const message = [
      `The Orchester native package for ${target.platform}/${target.arch} is missing: ${target.packageName}`,
      'Install the matching version with one of:',
      `  npm install -g ${metaPackage}`,
      `  pnpm add -g ${metaPackage}`,
      `  yarn global add ${metaPackage}`,
      `  bun add -g ${metaPackage}`,
    ].join('\n');

    super(message);
    this.name = 'MissingNativePackageError';
    this.code = 'ORCHESTER_MISSING_NATIVE_PACKAGE';
    this.packageName = target.packageName;
  }
}

function resolveNativeBinary(target, resolvePackageJson = require.resolve) {
  let packageManifest;

  try {
    packageManifest = resolvePackageJson(`${target.packageName}/package.json`);
  } catch {
    throw new MissingNativePackageError(target);
  }

  return path.join(path.dirname(packageManifest), target.binaryPath);
}

function runNativeCli(options = {}) {
  const {
    args = process.argv.slice(2),
    cwd = process.cwd(),
    env = process.env,
    parentProcess = process,
    platform = process.platform,
    resolvePackageJson = require.resolve,
    spawn = spawnProcess,
    target = resolveTarget(),
    writeError = (message) => parentProcess.stderr.write(`${message}\n`),
  } = options;
  const binary = resolveNativeBinary(target, resolvePackageJson);

  return new Promise((resolve) => {
    let child;
    let settled = false;
    const forwardingSignals = platform === 'win32'
      ? ['SIGINT', 'SIGTERM']
      : ['SIGINT', 'SIGTERM', 'SIGHUP'];
    const forwardingListeners = new Map();

    const cleanup = () => {
      for (const [signal, listener] of forwardingListeners) {
        parentProcess.removeListener(signal, listener);
      }
      forwardingListeners.clear();

      if (child) {
        child.removeListener('error', onError);
        child.removeListener('exit', onExit);
      }
    };

    const settleCode = (code) => {
      if (settled) return;
      settled = true;
      cleanup();
      parentProcess.exitCode = code;
      resolve(code);
    };

    const settleSignal = (signal) => {
      if (settled) return;
      settled = true;
      cleanup();

      if (platform === 'win32') {
        parentProcess.exitCode = 1;
        resolve(1);
        return;
      }

      try {
        parentProcess.kill(parentProcess.pid, signal);
        resolve(null);
      } catch {
        parentProcess.exitCode = 1;
        resolve(1);
      }
    };

    const onError = () => {
      if (settled) return;
      writeError(SPAWN_ERROR_MESSAGE);
      settleCode(1);
    };

    const onExit = (code, signal) => {
      if (signal) {
        settleSignal(signal);
        return;
      }

      settleCode(code ?? 1);
    };

    try {
      child = spawn(binary, args, {
        cwd,
        env,
        shell: false,
        stdio: 'inherit',
      });
    } catch {
      writeError(SPAWN_ERROR_MESSAGE);
      settleCode(1);
      return;
    }

    child.once('error', onError);
    child.once('exit', onExit);

    for (const signal of forwardingSignals) {
      const listener = () => {
        try {
          child.kill(signal);
        } catch {
          settleCode(1);
        }
      };
      forwardingListeners.set(signal, listener);
      parentProcess.on(signal, listener);
    }
  });
}

module.exports = {
  MissingNativePackageError,
  SPAWN_ERROR_MESSAGE,
  resolveNativeBinary,
  runNativeCli,
};
