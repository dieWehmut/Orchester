'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');

const packageJson = require('../package.json');
const targets = require('../targets.json');
const {
  UnsupportedTargetError,
  resolveTarget,
} = require('../lib/target.cjs');

const expectedTargets = [
  {
    platform: 'linux',
    arch: 'x64',
    packageName: '@orchester/cli-linux-x64',
    rustTarget: 'x86_64-unknown-linux-musl',
    binaryPath: 'bin/orchester',
  },
  {
    platform: 'linux',
    arch: 'arm64',
    packageName: '@orchester/cli-linux-arm64',
    rustTarget: 'aarch64-unknown-linux-musl',
    binaryPath: 'bin/orchester',
  },
  {
    platform: 'darwin',
    arch: 'x64',
    packageName: '@orchester/cli-darwin-x64',
    rustTarget: 'x86_64-apple-darwin',
    binaryPath: 'bin/orchester',
  },
  {
    platform: 'darwin',
    arch: 'arm64',
    packageName: '@orchester/cli-darwin-arm64',
    rustTarget: 'aarch64-apple-darwin',
    binaryPath: 'bin/orchester',
  },
  {
    platform: 'win32',
    arch: 'x64',
    packageName: '@orchester/cli-win32-x64',
    rustTarget: 'x86_64-pc-windows-msvc',
    binaryPath: 'bin/orchester.exe',
  },
  {
    platform: 'win32',
    arch: 'arm64',
    packageName: '@orchester/cli-win32-arm64',
    rustTarget: 'aarch64-pc-windows-msvc',
    binaryPath: 'bin/orchester.exe',
  },
];

test('resolves every supported Node platform and architecture', () => {
  for (const expected of expectedTargets) {
    assert.deepEqual(resolveTarget(expected.platform, expected.arch), expected);
  }
});

test('throws a typed error for an unsupported target', () => {
  assert.throws(
    () => resolveTarget('freebsd', 'x64'),
    (error) => {
      assert.ok(error instanceof UnsupportedTargetError);
      assert.equal(error.name, 'UnsupportedTargetError');
      assert.equal(error.code, 'ORCHESTER_UNSUPPORTED_TARGET');
      assert.equal(error.platform, 'freebsd');
      assert.equal(error.arch, 'x64');
      return true;
    },
  );
});

test('target manifest contains exactly the unique supported targets', () => {
  assert.deepEqual(targets, expectedTargets);
  assert.equal(
    new Set(targets.map(({ platform, arch }) => `${platform}/${arch}`)).size,
    targets.length,
  );
  assert.equal(
    new Set(targets.map(({ packageName }) => packageName)).size,
    targets.length,
  );
  assert.equal(
    new Set(targets.map(({ rustTarget }) => rustTarget)).size,
    targets.length,
  );
});

test('meta package pins every platform package without lifecycle scripts', () => {
  assert.equal(packageJson.name, '@orchester/cli');
  assert.equal(packageJson.version, '0.1.0');
  assert.equal(packageJson.type, 'commonjs');
  assert.deepEqual(packageJson.engines, { node: '>=18' });
  assert.deepEqual(packageJson.files, ['bin', 'lib', 'targets.json']);
  assert.deepEqual(packageJson.publishConfig, { access: 'public' });
  assert.equal(packageJson.bin.orchester, 'bin/orchester.cjs');

  assert.deepEqual(
    packageJson.optionalDependencies,
    Object.fromEntries(
      expectedTargets.map(({ packageName }) => [packageName, '0.1.0']),
    ),
  );

  for (const lifecycle of ['preinstall', 'install', 'postinstall']) {
    assert.equal(
      Object.hasOwn(packageJson.scripts ?? {}, lifecycle),
      false,
      `unexpected ${lifecycle} lifecycle script`,
    );
  }

  assert.equal(path.posix.normalize(packageJson.bin.orchester), packageJson.bin.orchester);
});
