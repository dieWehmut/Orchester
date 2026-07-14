'use strict';

const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { EventEmitter } = require('node:events');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const packageJson = require('../package.json');
const {
  MissingNativePackageError,
  SPAWN_ERROR_MESSAGE,
  resolveNativeBinary,
  runNativeCli,
} = require('../lib/process.cjs');
const { resolveTarget } = require('../lib/target.cjs');

const SPAWN_FAILURE = 'Failed to start the Orchester native executable.';

class FakeChildProcess extends EventEmitter {
  constructor() {
    super();
    this.forwardedSignals = [];
  }

  kill(signal) {
    this.forwardedSignals.push(signal);
    return true;
  }
}

class FakeParentProcess extends EventEmitter {
  constructor(pid = 4242) {
    super();
    this.exitCode = undefined;
    this.pid = pid;
    this.resentSignals = [];
  }

  kill(pid, signal) {
    this.resentSignals.push({ pid, signal });
    return true;
  }
}

function expectedMissingPackageMessage(target) {
  const metaPackage = `${packageJson.name}@${packageJson.version}`;

  return [
    `The Orchester native package for ${target.platform}/${target.arch} is missing: ${target.packageName}`,
    'Install the matching version with one of:',
    `  npm install -g ${metaPackage}`,
    `  pnpm add -g ${metaPackage}`,
    `  yarn global add ${metaPackage}`,
    `  bun add -g ${metaPackage}`,
  ].join('\n');
}

function startWithFakeChild({
  platform = 'linux',
  args = [],
  cwd = process.cwd(),
  env = process.env,
  writeError,
} = {}) {
  const child = new FakeChildProcess();
  const parentProcess = new FakeParentProcess();
  const target = {
    platform,
    arch: 'x64',
    packageName: '@orchester/cli-test-x64',
    binaryPath: platform === 'win32' ? 'bin/orchester.exe' : 'bin/orchester',
  };
  const packageManifest = path.join(os.tmpdir(), 'orchester native package', 'package.json');
  const completion = runNativeCli({
    args,
    cwd,
    env,
    parentProcess,
    platform,
    resolvePackageJson: () => packageManifest,
    spawn: () => child,
    target,
    writeError,
  });

  return { child, completion, parentProcess, target };
}

test('resolves the manifest binary through the injected package.json resolver', () => {
  const target = {
    platform: 'linux',
    arch: 'x64',
    packageName: '@orchester/cli-linux-x64',
    binaryPath: 'bin/orchester',
  };
  const packageManifest = path.join(os.tmpdir(), 'native package with spaces', 'package.json');
  const requests = [];

  const binary = resolveNativeBinary(target, (request) => {
    requests.push(request);
    return packageManifest;
  });

  assert.equal(binary, path.join(path.dirname(packageManifest), target.binaryPath));
  assert.deepEqual(requests, [`${target.packageName}/package.json`]);
});

test('converts resolver failures to a safe typed missing-package error', () => {
  const target = resolveTarget('linux', 'x64');
  const privatePath = path.join(os.tmpdir(), 'private dependency cache', 'package.json');
  const resolverFailure = new Error(`Cannot load ${privatePath}`);

  assert.throws(
    () => resolveNativeBinary(target, () => {
      throw resolverFailure;
    }),
    (error) => {
      assert.ok(error instanceof MissingNativePackageError);
      assert.equal(error.name, 'MissingNativePackageError');
      assert.equal(error.code, 'ORCHESTER_MISSING_NATIVE_PACKAGE');
      assert.equal(error.packageName, target.packageName);
      assert.equal(error.message, expectedMissingPackageMessage(target));
      assert.equal(Object.hasOwn(error, 'cause'), false);
      assert.doesNotMatch(error.message, new RegExp(privatePath.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
      assert.doesNotMatch(error.stack, new RegExp(privatePath.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
      return true;
    },
  );
});

test('spawns the native binary with exact arguments, cwd, env, and inherited stdio', async () => {
  const args = [
    'value with spaces',
    '"double quotes"',
    "single ' quotes",
    'alpha&beta',
    '$(printf injected)',
  ];
  const cwd = path.join(os.tmpdir(), 'working directory with spaces');
  const env = { PATH: 'literal path', VALUE: '$(still literal)&more' };
  const child = new FakeChildProcess();
  const parentProcess = new FakeParentProcess();
  const target = {
    platform: 'linux',
    arch: 'x64',
    packageName: '@orchester/cli-linux-x64',
    binaryPath: 'bin/orchester',
  };
  const packageManifest = path.join(os.tmpdir(), 'native package', 'package.json');
  const calls = [];

  const completion = runNativeCli({
    args,
    cwd,
    env,
    parentProcess,
    platform: 'linux',
    resolvePackageJson: () => packageManifest,
    spawn(binary, spawnedArgs, options) {
      calls.push({ binary, spawnedArgs, options });
      return child;
    },
    target,
  });

  assert.equal(calls.length, 1);
  assert.equal(calls[0].binary, path.join(path.dirname(packageManifest), target.binaryPath));
  assert.strictEqual(calls[0].spawnedArgs, args);
  assert.deepEqual(calls[0].options, {
    cwd,
    env,
    shell: false,
    stdio: 'inherit',
  });

  child.emit('exit', 0, null);
  assert.equal(await completion, 0);
});

test('forwards parent signals and removes all forwarding listeners after exit', async () => {
  const { child, completion, parentProcess } = startWithFakeChild({ platform: 'linux' });

  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    assert.equal(parentProcess.listenerCount(signal), 1);
    parentProcess.emit(signal);
  }
  assert.deepEqual(child.forwardedSignals, ['SIGINT', 'SIGTERM', 'SIGHUP']);

  child.emit('exit', 0, null);
  assert.equal(await completion, 0);

  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    assert.equal(parentProcess.listenerCount(signal), 0);
  }
  assert.equal(child.listenerCount('error'), 0);
  assert.equal(child.listenerCount('exit'), 0);
});

test('forwards SIGINT and SIGTERM but does not register SIGHUP on Windows', async () => {
  const { child, completion, parentProcess } = startWithFakeChild({ platform: 'win32' });

  assert.equal(parentProcess.listenerCount('SIGINT'), 1);
  assert.equal(parentProcess.listenerCount('SIGTERM'), 1);
  assert.equal(parentProcess.listenerCount('SIGHUP'), 0);
  parentProcess.emit('SIGINT');
  parentProcess.emit('SIGTERM');
  assert.deepEqual(child.forwardedSignals, ['SIGINT', 'SIGTERM']);

  child.emit('exit', 0, null);
  assert.equal(await completion, 0);
});

test('mirrors child exit codes zero and 42', async () => {
  for (const code of [0, 42]) {
    const { child, completion, parentProcess } = startWithFakeChild();

    child.emit('exit', code, null);

    assert.equal(await completion, code);
    assert.equal(parentProcess.exitCode, code);
  }
});

test('re-sends a child termination signal to the parent process on POSIX', async () => {
  const { child, completion, parentProcess } = startWithFakeChild({ platform: 'linux' });

  child.emit('exit', null, 'SIGTERM');

  assert.equal(await completion, null);
  assert.equal(parentProcess.exitCode, undefined);
  assert.deepEqual(parentProcess.resentSignals, [{ pid: parentProcess.pid, signal: 'SIGTERM' }]);
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    assert.equal(parentProcess.listenerCount(signal), 0);
  }
});

test('uses exit code one when re-sending a POSIX child signal throws', async () => {
  const { child, completion, parentProcess } = startWithFakeChild({ platform: 'linux' });
  const privatePath = path.join(os.tmpdir(), 'private process state');
  parentProcess.kill = () => {
    throw new Error(`kill failed at ${privatePath}`);
  };

  assert.doesNotThrow(() => child.emit('exit', null, 'SIGTERM'));

  assert.equal(await completion, 1);
  assert.equal(parentProcess.exitCode, 1);
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    assert.equal(parentProcess.listenerCount(signal), 0);
  }
});

test('uses exit code one when a child signal cannot be mirrored on Windows', async () => {
  const { child, completion, parentProcess } = startWithFakeChild({ platform: 'win32' });

  child.emit('exit', null, 'SIGTERM');

  assert.equal(await completion, 1);
  assert.equal(parentProcess.exitCode, 1);
  assert.deepEqual(parentProcess.resentSignals, []);
});

test('uses exit code one when forwarding a parent signal to the child throws', async () => {
  const { child, completion, parentProcess } = startWithFakeChild({ platform: 'linux' });
  const privatePath = path.join(os.tmpdir(), 'private child state');
  child.kill = () => {
    throw new Error(`child kill failed at ${privatePath}`);
  };

  assert.doesNotThrow(() => parentProcess.emit('SIGINT'));

  assert.equal(await completion, 1);
  assert.equal(parentProcess.exitCode, 1);
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    assert.equal(parentProcess.listenerCount(signal), 0);
  }
  assert.equal(child.listenerCount('error'), 0);
  assert.equal(child.listenerCount('exit'), 0);
});

test('reports an asynchronous spawn error safely and settles only once', async () => {
  const errors = [];
  const { child, completion, parentProcess } = startWithFakeChild({
    writeError(message) {
      errors.push(message);
    },
  });
  const privatePath = path.join(os.tmpdir(), 'private executable path', 'orchester');

  child.emit('error', new Error(`spawn ${privatePath} ENOENT`));
  child.emit('exit', 42, null);

  assert.equal(await completion, 1);
  assert.equal(parentProcess.exitCode, 1);
  assert.equal(SPAWN_ERROR_MESSAGE, SPAWN_FAILURE);
  assert.deepEqual(errors, [SPAWN_FAILURE]);
  assert.doesNotMatch(errors.join('\n'), new RegExp(privatePath.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')));
  for (const signal of ['SIGINT', 'SIGTERM', 'SIGHUP']) {
    assert.equal(parentProcess.listenerCount(signal), 0);
  }
  assert.equal(child.listenerCount('error'), 0);
  assert.equal(child.listenerCount('exit'), 0);
});

test('real Node child preserves dangerous arguments and the launcher mirrors exit 42', (t) => {
  const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-process-test-'));
  t.after(() => fs.rmSync(temporaryDirectory, { force: true, recursive: true }));

  const outputPath = path.join(temporaryDirectory, 'arguments.json');
  const wrapperPath = path.join(temporaryDirectory, 'launcher.cjs');
  const processLibrary = path.resolve(__dirname, '../lib/process.cjs');
  const dangerousArguments = [
    'value with spaces',
    '"double quotes"',
    "single ' quotes",
    'alpha&beta',
    '$(printf injected)',
    '$HOME',
    'semi;colon',
  ];
  const nativeHarness = [
    "const fs = require('node:fs');",
    'fs.writeFileSync(process.argv[1], JSON.stringify(process.argv.slice(2)));',
    'process.exit(42);',
  ].join('');
  const wrapper = [
    "'use strict';",
    "const path = require('node:path');",
    `const { runNativeCli } = require(${JSON.stringify(processLibrary)});`,
    `const args = ${JSON.stringify(['-e', nativeHarness, outputPath, ...dangerousArguments])};`,
    'runNativeCli({',
    '  args,',
    "  platform: process.platform,",
    "  resolvePackageJson(request) {",
    "    if (request !== '@orchester/test-native/package.json') throw new Error('unexpected request');",
    "    return path.join(path.dirname(process.execPath), 'package.json');",
    '  },',
    "  target: { packageName: '@orchester/test-native', binaryPath: path.basename(process.execPath) },",
    '});',
  ].join('\n');
  fs.writeFileSync(wrapperPath, wrapper);

  const result = spawnSync(process.execPath, [wrapperPath], {
    cwd: temporaryDirectory,
    encoding: 'utf8',
    env: {
      ...process.env,
      ORCHESTER_BINARY_PATH: path.join(temporaryDirectory, 'must-not-be-used'),
    },
    timeout: 10_000,
  });

  assert.equal(result.error, undefined);
  assert.equal(result.signal, null);
  assert.equal(result.status, 42);
  assert.equal(result.stdout, '');
  assert.equal(result.stderr, '');
  assert.deepEqual(JSON.parse(fs.readFileSync(outputPath, 'utf8')), dangerousArguments);
});

test('source bin reports only the missing platform package and exact install commands', () => {
  const target = resolveTarget();
  const binPath = path.resolve(__dirname, '../bin/orchester.cjs');
  const environment = {
    ...process.env,
    ORCHESTER_BINARY_PATH: process.execPath,
  };
  delete environment.NODE_PATH;

  const result = spawnSync(process.execPath, [binPath], {
    cwd: path.dirname(binPath),
    encoding: 'utf8',
    env: environment,
    timeout: 10_000,
  });

  assert.equal(result.error, undefined);
  assert.equal(result.signal, null);
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.equal(result.stderr, `${expectedMissingPackageMessage(target)}\n`);
  assert.doesNotMatch(result.stderr, /(?:\n\s+at\s|process\.cjs|node:internal|Error:)/);
});
