import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';

import { SUPPORTED_MANAGERS } from './plans.mjs';

const WINDOWS_SHELL_METACHARS = /[\u0000-\u001f\u007f"%!^&|<>]/;

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function assertSafeCmdToken(value, label) {
  if (typeof value !== 'string' || value.length === 0 || WINDOWS_SHELL_METACHARS.test(value)) {
    fail('ORCHESTER_NPM_UNSAFE_COMMAND', `${label} contains unsafe Windows shell characters`);
  }
}

function quoteCmdToken(value, label) {
  assertSafeCmdToken(value, label);
  return `"${value}"`;
}

function resolveOnPath(name, env = process.env, platform = process.platform) {
  const pathValue = env.PATH ?? env.Path ?? '';
  const entries = String(pathValue).split(path.delimiter).filter(Boolean);
  const suffixes = platform === 'win32' ? ['.cmd', '.exe', '.bat'] : [''];
  for (const entry of entries) {
    for (const suffix of suffixes) {
      const candidate = path.resolve(entry, `${name}${suffix}`);
      let status;
      try {
        status = fs.statSync(candidate);
      } catch {
        continue;
      }
      if (!status.isFile()) continue;
      if (platform !== 'win32' && (status.mode & 0o111) === 0) continue;
      return candidate;
    }
  }
  return undefined;
}

function cachedYarnDescriptor(env, platform) {
  const candidates = [];
  if (env.COREPACK_HOME) candidates.push(path.join(env.COREPACK_HOME, 'v1', 'yarn', '1.22.22', 'bin', 'yarn.js'));
  if (platform === 'win32' && env.LOCALAPPDATA) {
    candidates.push(path.join(env.LOCALAPPDATA, 'node', 'corepack', 'v1', 'yarn', '1.22.22', 'bin', 'yarn.js'));
  }
  const cacheHome = env.XDG_CACHE_HOME || (platform === 'win32'
    ? env.LOCALAPPDATA
    : path.join(env.HOME || os.homedir(), '.cache'));
  if (cacheHome) candidates.push(path.join(cacheHome, 'node', 'corepack', 'v1', 'yarn', '1.22.22', 'bin', 'yarn.js'));
  if (platform === 'darwin' && env.HOME) {
    candidates.push(path.join(env.HOME, 'Library', 'Caches', 'node', 'corepack', 'v1', 'yarn', '1.22.22', 'bin', 'yarn.js'));
  }

  for (const candidate of [...new Set(candidates)]) {
    let manifest;
    try {
      manifest = JSON.parse(fs.readFileSync(path.join(path.dirname(path.dirname(candidate)), 'package.json'), 'utf8'));
    } catch {
      continue;
    }
    if (manifest.version !== '1.22.22') continue;
    try {
      if (!fs.statSync(candidate).isFile()) continue;
    } catch {
      continue;
    }
    return {
      name: 'yarn',
      command: process.execPath,
      prefixArgs: [candidate],
      commandKind: 'node',
      version: manifest.version,
      source: 'corepack-cache',
    };
  }
  return undefined;
}

function descriptorFor(name, env, platform) {
  if (name === 'yarn') {
    const cached = cachedYarnDescriptor(env, platform);
    if (cached) return cached;
    const yarnPath = resolveOnPath('yarn', env, platform);
    if (yarnPath) {
      return {
        name,
        command: yarnPath,
        prefixArgs: [],
        commandKind: path.extname(yarnPath).toLowerCase() === '.cmd' ? 'cmd' : 'direct',
        source: 'path',
      };
    }
    return undefined;
  }
  const executable = resolveOnPath(name, env, platform);
  if (!executable) return undefined;
  return {
    name,
    command: executable,
    prefixArgs: [],
    commandKind: platform === 'win32' && ['.cmd', '.bat'].includes(path.extname(executable).toLowerCase())
      ? 'cmd'
      : 'direct',
  };
}

/** Discover only installed managers; no package-manager command can download anything here. */
export function discoverPackageManagers({ env = process.env, platform = process.platform } = {}) {
  const discovered = new Map();
  for (const name of SUPPORTED_MANAGERS) {
    const descriptor = descriptorFor(name, env, platform);
    if (descriptor) discovered.set(name, descriptor);
  }
  return discovered;
}

function comspec(env) {
  const candidate = env.ComSpec || env.COMSPEC || (env.SystemRoot
    ? path.join(env.SystemRoot, 'System32', 'cmd.exe')
    : undefined);
  if (!candidate) fail('ORCHESTER_NPM_SHELL_UNAVAILABLE', 'Windows command shell is unavailable');
  return candidate;
}

export function invokeCommand(descriptor, args, {
  env = process.env,
  cwd = process.cwd(),
  timeout = 120_000,
  spawn = spawnSync,
} = {}) {
  const prefixArgs = descriptor.prefixArgs || [];
  const fullArgs = [...prefixArgs, ...args];
  const options = {
    cwd,
    env,
    encoding: 'utf8',
    timeout,
    windowsHide: true,
    input: undefined,
  };
  let result;
  if (descriptor.commandKind === 'cmd') {
    const tokens = [descriptor.command, ...fullArgs];
    const commandLine = tokens.map((token, index) => quoteCmdToken(token, index === 0 ? 'manager path' : 'manager argument')).join(' ');
    result = spawn(
      comspec(env),
      ['/d', '/v:off', '/s', '/c', `"${commandLine}"`],
      { ...options, windowsVerbatimArguments: true },
    );
  } else {
    result = spawn(descriptor.command, fullArgs, { ...options, shell: false });
  }
  return {
    command: descriptor.command,
    status: result.status ?? 1,
    signal: result.signal,
    stdout: String(result.stdout ?? ''),
    stderr: String(result.stderr ?? ''),
    error: result.error,
    args: fullArgs,
  };
}

export function commandSummary(descriptor, args) {
  return [descriptor.command, ...(descriptor.prefixArgs || []), ...args].join(' ');
}
