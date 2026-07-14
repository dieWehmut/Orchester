import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  FIXTURE_MARKER,
  buildManagerPlan,
  createIsolatedEnvironment,
  discoverPackageManagers,
  invokeCommand,
  parseCommandLine,
  runGlobalInstallSmoke,
  runManagerSmoke,
} from './install.mjs';

const managers = discoverPackageManagers();

function temporaryRoot(t, name) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), `orchester-${name}-`));
  t.after(() => fs.rmSync(root, { force: true, recursive: true }));
  return root;
}

test('builds exact isolated global install and removal commands', (t) => {
  const root = temporaryRoot(t, 'npm-install-plan');
  const sandbox = createIsolatedEnvironment(path.join(root, 'sandbox'), {
    PATH: process.env.PATH,
    SystemRoot: process.env.SystemRoot,
  });
  const tarball = path.join(root, 'fixture.tgz');

  const npm = buildManagerPlan('npm', sandbox, tarball);
  assert.deepEqual(npm.installArgs.slice(0, 3), ['install', '-g', tarball]);
  assert.deepEqual(npm.removeArgs.slice(0, 3), ['uninstall', '-g', 'orchester-install-smoke-fixture']);
  assert.ok(npm.installArgs.includes('--offline'));
  assert.ok(npm.installArgs.includes('--ignore-scripts'));

  const pnpm = buildManagerPlan('pnpm', sandbox, tarball);
  assert.deepEqual(pnpm.installArgs.slice(0, 3), ['add', '--global', tarball]);
  assert.deepEqual(pnpm.removeArgs, [
    'remove',
    '--global',
    '--global-dir',
    sandbox.globalDirectory,
    'orchester-install-smoke-fixture',
  ]);
  for (const forbidden of ['--global-bin-dir', '--offline', '--ignore-scripts']) {
    assert.equal(pnpm.removeArgs.includes(forbidden), false);
  }

  const yarn = buildManagerPlan('yarn', sandbox, tarball);
  assert.deepEqual(yarn.installArgs.slice(0, 3), ['global', 'add', tarball]);
  assert.deepEqual(yarn.removeArgs.slice(0, 3), ['global', 'remove', 'orchester-install-smoke-fixture']);
  assert.ok(yarn.installArgs.includes('--offline'));
  assert.ok(yarn.installArgs.includes('--ignore-scripts'));

  const bun = buildManagerPlan('bun', sandbox, tarball);
  assert.deepEqual(bun.installArgs.slice(0, 3), ['add', '--global', tarball]);
  assert.deepEqual(bun.removeArgs.slice(0, 3), ['remove', '--global', 'orchester-install-smoke-fixture']);
  assert.ok(bun.installArgs.includes('--ignore-scripts'));
  // Bun must save its isolated global manifest or `bun remove --global`
  // cannot identify and remove a local tarball installation.
  assert.equal(bun.installArgs.includes('--no-save'), false);
  assert.ok(bun.installArgs.includes('--cache-dir'));
  assert.ok(bun.installArgs.includes('--cwd'));
  assert.equal(bun.installArgs.includes('--offline'), false);
});

test('isolates every package-manager home, prefix, store, cache, and registry', (t) => {
  const root = temporaryRoot(t, 'npm-install-env');
  const original = {
    ...process.env,
    HOME: 'outside-home',
    USERPROFILE: 'outside-profile',
    APPDATA: 'outside-appdata',
    LOCALAPPDATA: 'outside-localappdata',
    NODE_OPTIONS: '--require=outside-hook.cjs',
    HTTPS_PROXY: 'https://user:secret@outside.invalid',
    NPM_TOKEN: 'outside-secret',
    YARN_RC_FILENAME: 'outside-yarnrc.yml',
    npm_config_userconfig: 'outside-npmrc',
  };

  const sandbox = createIsolatedEnvironment(path.join(root, 'sandbox'), original);

  for (const variable of ['HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA']) {
    assert.ok(path.resolve(sandbox.env[variable]).startsWith(path.resolve(sandbox.root)));
    assert.notEqual(sandbox.env[variable], original[variable]);
  }
  for (const location of [
    sandbox.prefix,
    sandbox.globalDirectory,
    sandbox.binDirectory,
    sandbox.storeDirectory,
    sandbox.cacheDirectory,
    sandbox.bunInstall,
    sandbox.bunGlobalDirectory,
  ]) {
    assert.ok(path.resolve(location).startsWith(path.resolve(sandbox.root)));
    assert.equal(fs.statSync(location).isDirectory(), true);
  }
  assert.equal(sandbox.env.BUN_INSTALL, sandbox.bunInstall);
  assert.equal(sandbox.env.PNPM_HOME, sandbox.binDirectory);
  assert.match(sandbox.env.npm_config_registry, /^http:\/\/127\.0\.0\.1:\d+\/$/);
  for (const variable of [
    'NODE_OPTIONS',
    'HTTPS_PROXY',
    'NPM_TOKEN',
    'YARN_RC_FILENAME',
  ]) {
    assert.equal(sandbox.env[variable], undefined);
  }
  assert.equal(sandbox.env.npm_config_userconfig, sandbox.userConfig);
  for (const variable of ['TEMP', 'TMP', 'TMPDIR']) {
    assert.ok(path.resolve(sandbox.env[variable]).startsWith(path.resolve(sandbox.root)));
  }
});

test('writes scalar npm configuration values without path coercion', (t) => {
  const root = temporaryRoot(t, 'npm-install-config');
  const sandbox = createIsolatedEnvironment(path.join(root, 'sandbox'), {
    PATH: process.env.PATH,
    SystemRoot: process.env.SystemRoot,
  });
  const userConfig = fs.readFileSync(sandbox.userConfig, 'utf8').split(/\r?\n/);
  const globalConfig = fs.readFileSync(sandbox.globalConfig, 'utf8').split(/\r?\n/);

  assert.ok(userConfig.includes(`registry=${sandbox.registry}`));
  assert.ok(userConfig.includes('ignore-scripts=true'));
  assert.ok(userConfig.includes('audit=false'));
  assert.ok(userConfig.includes(`cache=${sandbox.cacheDirectory}`));
  assert.ok(globalConfig.includes(`registry=${sandbox.registry}`));
  assert.equal(userConfig.some((line) => line.includes('http:/') && line.includes('Orchester')), false);
});

test('quotes Windows cmd manager tokens verbatim and rejects command separators', () => {
  let invocation;
  const result = invokeCommand(
    {
      command: 'C:\\Program Files (x86)\\nodejs\\npm.cmd',
      commandKind: 'cmd',
      prefixArgs: [],
    },
    ['install', '-g', 'C:\\Temp Root\\fixture.tgz'],
    {
      cwd: 'C:\\Temp Root',
      env: { ComSpec: 'C:\\Windows\\System32\\cmd.exe' },
      spawn(command, args, options) {
        invocation = { args, command, options };
        return { status: 0, stderr: '', stdout: '' };
      },
    },
  );

  assert.equal(result.status, 0);
  assert.equal(invocation.options.windowsVerbatimArguments, true);
  assert.deepEqual(invocation.args.slice(0, 4), ['/d', '/v:off', '/s', '/c']);
  assert.match(invocation.args[4], /^""C:\\Program Files \(x86\)\\nodejs\\npm\.cmd"/);
  assert.match(invocation.args[4], /"C:\\Temp Root\\fixture\.tgz""$/);

  assert.throws(
    () => invokeCommand(
      { command: 'C:\\node\\npm.cmd', commandKind: 'cmd', prefixArgs: [] },
      ['install', 'fixture.tgz & whoami'],
      {
        env: { ComSpec: 'C:\\Windows\\System32\\cmd.exe' },
        spawn() {
          throw new Error('must not spawn');
        },
      },
    ),
    (error) => error.code === 'ORCHESTER_NPM_UNSAFE_COMMAND',
  );
});

test('keeps one Windows PATH binding and prefixes the Node runtime directory', (t) => {
  const root = temporaryRoot(t, 'npm-install-path');
  const sandbox = createIsolatedEnvironment(path.join(root, 'sandbox'), {
    Path: 'C:\\outside\\bin',
    SystemRoot: process.env.SystemRoot,
  });
  const pathKeys = Object.keys(sandbox.env).filter((key) => key.toLowerCase() === 'path');

  assert.deepEqual(pathKeys.length, 1);
  assert.equal(pathKeys[0], 'Path');
  assert.equal(sandbox.env.Path.split(path.delimiter)[0], sandbox.binDirectory);
  assert.equal(
    sandbox.env.Path.split(path.delimiter).includes(path.dirname(process.execPath)),
    true,
  );
});

test('discovers cached Yarn Classic directly without invoking Corepack', () => {
  const yarn = managers.get('yarn');
  if (!yarn) return;

  assert.equal(yarn.version, '1.22.22');
  if (yarn.source === 'corepack-cache') {
    assert.equal(path.resolve(yarn.command), path.resolve(process.execPath));
    assert.match(yarn.prefixArgs[0], /corepack[\\/]v1[\\/]yarn[\\/]1\.22\.22/);
  }
});

test('parses the require-all switch and rejects all other CLI input', () => {
  assert.deepEqual(parseCommandLine([]), { requireAll: false });
  assert.deepEqual(parseCommandLine(['--require-all']), { requireAll: true });
  assert.throws(
    () => parseCommandLine(['--unknown']),
    (error) => error.code === 'ORCHESTER_NPM_USAGE',
  );
});

for (const name of ['npm', 'pnpm', 'yarn', 'bun']) {
  const manager = managers.get(name);
  test(`${name} installs, executes, and removes the fixture globally`, {
    skip: manager ? false : `${name} is not installed or no supported offline executable was found`,
    timeout: 60_000,
  }, (t) => {
    const root = temporaryRoot(t, `npm-install-${name}`);
    const managerRoot = name === 'bun'
      ? path.join(root, 'run')
      : path.join(root, 'space root', 'run');
    const result = runManagerSmoke({ manager, root: managerRoot });

    assert.equal(result.name, name);
    assert.equal(result.packageRemoved, true);
    if (result.executionPassed) {
      assert.equal(result.marker, FIXTURE_MARKER);
    } else {
      assert.equal(name, 'bun');
      assert.equal(process.platform, 'win32');
      assert.equal(process.arch, 'arm64');
      assert.equal(result.executionFailure.code, 'ORCHESTER_NPM_FIXTURE_FAILED');
      t.diagnostic(`Bun's generated x64 Windows entry is not executable on ${process.arch}: ${result.executionFailure.code}`);
    }
    if (result.shimRemoved) {
      assert.equal(fs.existsSync(result.shim), false);
    } else {
      assert.equal(name, 'bun');
      assert.equal(process.platform, 'win32');
      assert.ok(result.residualShims.length > 0);
      t.diagnostic(`Bun ${manager.version ?? 'unknown'} left owned Windows shims: ${result.residualShims.join(', ')}`);
    }
    if (name === 'bun' && process.platform === 'win32') {
      assert.equal(path.extname(result.shim).toLowerCase(), '.exe');
    }
    assert.ok(path.resolve(result.shim).startsWith(path.resolve(root)));
  });
}

test('require-all reports every missing package manager before creating sandboxes', (t) => {
  const root = path.join(temporaryRoot(t, 'npm-install-required'), 'not-created');

  assert.throws(
    () => runGlobalInstallSmoke({
      managers: new Map([['npm', managers.get('npm')]]),
      requireAll: true,
      root,
    }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_MANAGER_MISSING');
      assert.match(error.message, /pnpm/);
      assert.match(error.message, /yarn/);
      assert.match(error.message, /bun/);
      return true;
    },
  );
  assert.equal(fs.existsSync(root), false);
});
