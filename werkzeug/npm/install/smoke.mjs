import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';

import {
  FIXTURE_MARKER,
  createFixtureTarball,
} from './fixture.mjs';
import {
  assertInside,
  createIsolatedEnvironment,
  ensureDirectory,
  mkdirFresh,
} from './environment.mjs';
import {
  commandSummary,
  discoverPackageManagers,
  invokeCommand,
} from './command.mjs';
import {
  buildManagerPlan,
  SUPPORTED_MANAGERS,
} from './plans.mjs';

function fail(code, message, details = {}) {
  const error = new Error(message);
  error.code = code;
  Object.assign(error, details);
  throw error;
}

function listShims(plan, root) {
  const names = process.platform === 'win32'
    ? plan.name === 'bun'
      ? [`${plan.packageName}.exe`, `${plan.packageName}.cmd`, `${plan.packageName}.bunx`, `${plan.packageName}.ps1`, plan.packageName]
      : [`${plan.packageName}.cmd`, `${plan.packageName}.exe`, `${plan.packageName}.ps1`, plan.packageName]
    : [plan.packageName];
  const shims = [];
  for (const directory of plan.shimRoots) {
    assertInside(root, directory, 'shim directory');
    for (const name of names) {
      const candidate = path.join(directory, name);
      let status;
      try {
        status = fs.lstatSync(candidate);
      } catch {
        continue;
      }
      if (!status.isFile() && !status.isSymbolicLink()) continue;
      if (status.isSymbolicLink()) {
        let real;
        try {
          real = fs.realpathSync(candidate);
        } catch {
          continue;
        }
        assertInside(root, real, 'shim target');
      }
      shims.push(candidate);
    }
  }
  return shims;
}

function bunxTarget(shim, sandbox, root) {
  let text;
  try {
    text = fs.readFileSync(shim).toString('utf16le');
  } catch (error) {
    fail('ORCHESTER_NPM_BUNX_INVALID', `cannot read Bun shim: ${shim}`, { cause: error });
  }
  const relativeTarget = text.split('\0', 1)[0].replace(/^\uFEFF/, '').replace(/^"|"$/g, '').trim();
  if (!relativeTarget || relativeTarget.includes('..')) {
    fail('ORCHESTER_NPM_BUNX_INVALID', `Bun shim target is invalid: ${shim}`);
  }
  const target = path.resolve(sandbox.bunInstall, relativeTarget);
  assertInside(root, target, 'Bun shim target');
  let status;
  try {
    status = fs.lstatSync(target);
  } catch (error) {
    fail('ORCHESTER_NPM_BUNX_INVALID', `Bun shim target is missing: ${target}`, { cause: error });
  }
  if (!status.isFile() || status.isSymbolicLink()) {
    fail('ORCHESTER_NPM_BUNX_INVALID', `Bun shim target is not a regular file: ${target}`);
  }
  return target;
}

function runShim(shim, sandbox, root, manager) {
  const extension = path.extname(shim).toLowerCase();
  const usesBunxNodeTarget = manager.name === 'bun'
    && process.platform === 'win32'
    && extension === '.bunx';
  const target = usesBunxNodeTarget ? bunxTarget(shim, sandbox, root) : undefined;
  const requiresWindowsShell = !usesBunxNodeTarget && process.platform === 'win32'
    && ['.cmd', '.bat'].includes(extension);
  const descriptor = usesBunxNodeTarget
    ? { command: process.execPath, prefixArgs: [target], commandKind: 'direct' }
    : {
      command: shim,
      prefixArgs: [],
      commandKind: requiresWindowsShell ? 'cmd' : 'direct',
    };
  const result = invokeCommand(descriptor, [], {
    env: sandbox.env,
    cwd: root,
  });
  if (result.status !== 0 || result.signal) {
    const output = `${result.stdout}${result.stderr}${result.error?.message ?? ''}`.trim().slice(-2_000);
    fail('ORCHESTER_NPM_FIXTURE_FAILED', `fixture shim failed (${result.status}): ${output}`, { result, shim });
  }
  const marker = result.stdout.trim();
  if (marker !== FIXTURE_MARKER) {
    fail('ORCHESTER_NPM_FIXTURE_MARKER', `fixture shim returned an unexpected marker: ${marker}`, { result });
  }
  return marker;
}

function assertCommandSucceeded(result, descriptor, args, phase) {
  if (result.status === 0 && !result.signal && !result.error) return;
  const output = `${result.stdout}${result.stderr}${result.error?.message ?? ''}`.trim().slice(-2_000);
  fail('ORCHESTER_NPM_MANAGER_FAILED', `${descriptor.name} ${phase} failed (${result.status}): ${output}`, {
    manager: descriptor.name,
    phase,
    command: commandSummary(descriptor, args),
    result,
  });
}

export function runManagerSmoke({ manager, root }) {
  if (!manager?.name || !manager.command) fail('ORCHESTER_NPM_MANAGER_INVALID', 'manager descriptor is invalid');
  const resolvedRoot = path.resolve(root);
  if (fs.existsSync(resolvedRoot)) fail('ORCHESTER_NPM_SANDBOX_EXISTS', `temporary sandbox already exists: ${resolvedRoot}`);
  ensureDirectory(path.dirname(resolvedRoot));
  mkdirFresh(resolvedRoot);
  const sandbox = createIsolatedEnvironment(path.join(resolvedRoot, 'environment'));
  const tarball = createFixtureTarball(path.join(resolvedRoot, 'fixture'));
  assertInside(resolvedRoot, tarball, 'fixture tarball');
  const plan = buildManagerPlan(manager.name, sandbox, tarball);

  const install = invokeCommand(manager, plan.installArgs, { env: sandbox.env, cwd: resolvedRoot });
  assertCommandSucceeded(install, manager, plan.installArgs, 'install');
  const installedShims = listShims(plan, resolvedRoot);
  const shim = installedShims[0];
  if (!shim) {
    fail('ORCHESTER_NPM_SHIM_MISSING', `${manager.name} did not create the fixture shim`, {
      manager: manager.name,
      install,
    });
  }
  let marker;
  let executionFailure;
  try {
    marker = runShim(shim, sandbox, resolvedRoot, manager);
  } catch (error) {
    if (typeof error?.code !== 'string' || !error.code.startsWith('ORCHESTER_NPM_')) throw error;
    executionFailure = { code: error.code, message: error.message };
  }

  const remove = invokeCommand(manager, plan.removeArgs, { env: sandbox.env, cwd: resolvedRoot });
  assertCommandSucceeded(remove, manager, plan.removeArgs, 'remove');
  const residualPackages = plan.packageRoots.filter((candidate) => fs.existsSync(candidate));
  if (residualPackages.length > 0) {
    fail('ORCHESTER_NPM_PACKAGE_RESIDUAL', `${manager.name} left the fixture package installed`, {
      manager: manager.name,
      residualPackages,
    });
  }
  const residualShims = listShims(plan, resolvedRoot);
  return {
    name: manager.name,
    marker,
    shim,
    executionPassed: executionFailure === undefined,
    executionFailure,
    shimRemoved: residualShims.length === 0,
    packageRemoved: true,
    residualShims,
    installArgs: plan.installArgs,
    removeArgs: plan.removeArgs,
    install,
    remove,
    sandbox,
  };
}

export function runGlobalInstallSmoke({
  managers = discoverPackageManagers(),
  requireAll = false,
  root,
} = {}) {
  const missing = SUPPORTED_MANAGERS.filter((name) => !managers.get(name));
  if (requireAll && missing.length > 0) {
    fail('ORCHESTER_NPM_MANAGER_MISSING', `required package managers are missing: ${missing.join(', ')}`);
  }

  const ownedRoot = root === undefined;
  const smokeRoot = path.resolve(root ?? fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-npm-install-')));
  if (!ownedRoot) {
    ensureDirectory(path.dirname(smokeRoot));
    mkdirFresh(smokeRoot);
  }
  const results = [];
  try {
    for (const name of SUPPORTED_MANAGERS) {
      const manager = managers.get(name);
      if (!manager) {
        results.push({
          name,
          status: 'skipped',
          reason: `${name} is not installed or no supported offline executable was found`,
        });
        continue;
      }
      const result = runManagerSmoke({ manager, root: path.join(smokeRoot, name) });
      results.push({
        ...result,
        status: result.executionPassed && result.shimRemoved ? 'passed' : 'failed',
      });
    }
    return { root: smokeRoot, results };
  } finally {
    if (ownedRoot) fs.rmSync(smokeRoot, { force: true, recursive: true });
  }
}
