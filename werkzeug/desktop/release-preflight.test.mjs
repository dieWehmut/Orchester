import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  cliPlatformPins,
  collectProblems,
  manifestVersion,
  parseArguments,
  pluginVersions,
  SEMVER,
} from './release-preflight.mjs';

/**
 * The preflight asks the questions the two release workflows ask, before either
 * is dispatched. What it judges arrives as arguments, so the judgement is tested
 * without a repository, a toolchain, or a network.
 */

const clean = {
  version: '9.9.9',
  desktopVersions: {
    'apps/desktop/package.json': '9.9.9',
    'apps/desktop/src-tauri/tauri.conf.json': '9.9.9',
    'apps/desktop/src-tauri/Cargo.toml': '9.9.9',
  },
  cliVersion: '9.9.9',
  platformPins: { '@orchester/cli-win32-x64': '9.9.9' },
  plugins: [{ directory: 'codex', package: '9.9.9', manifest: '9.9.9' }],
  cargo: { 'Cargo.toml': { packages: { 'orchester-netz': '9.9.9' } } },
  tags: {},
};

test('a consistent repository has nothing standing between it and a dispatch', () => {
  assert.deepEqual(collectProblems(clean), []);
});

test('every place the workflows validate is named when it disagrees', () => {
  const problems = collectProblems({
    ...clean,
    desktopVersions: { ...clean.desktopVersions, 'apps/desktop/src-tauri/tauri.conf.json': '9.9.8' },
    cliVersion: '9.9.8',
    platformPins: { '@orchester/cli-win32-x64': '9.9.8' },
    plugins: [
      { directory: 'codex', package: '9.9.9', manifest: '9.9.8' },
      { directory: 'claude', package: '9.9.8', manifest: '9.9.9' },
    ],
  });

  assert.equal(problems.length, 5);
  assert.ok(problems.some((problem) => problem.includes('tauri.conf.json is 9.9.8')));
  assert.ok(problems.some((problem) => problem.includes('apps/cli/package.json is 9.9.8')));
  assert.ok(problems.some((problem) => problem.includes('@orchester/cli-win32-x64 is pinned to 9.9.8')));
  assert.ok(problems.some((problem) => problem.includes('codex/orchester-plugin.json is 9.9.8')));
  assert.ok(problems.some((problem) => problem.includes('claude/package.json is 9.9.8')));
});

test('a workspace package that did not move with the cut is a problem', () => {
  const problems = collectProblems({
    ...clean,
    cargo: { 'Cargo.toml': { packages: { 'orchester-netz': '9.9.9', 'orchester-konsole': '9.9.8' } } },
  });

  assert.deepEqual(problems, ['Cargo.toml: orchester-konsole is 9.9.8, not 9.9.9']);
});

test('a lockfile that disagrees with its manifest is reported, not guessed at', () => {
  const problems = collectProblems({
    ...clean,
    cargo: { 'Cargo.toml': { error: 'Cargo.toml: cargo metadata --locked exited 101: the lock file needs to be updated' } },
  });

  assert.equal(problems.length, 1);
  assert.ok(problems[0]?.includes('the lock file needs to be updated'));
});

test('a tag that already names a different build stops the dispatch', () => {
  const problems = collectProblems({
    ...clean,
    tags: { 'v9.9.9': 'abc1234', 'desktop-v9.9.9': 'HEAD' },
  });

  assert.deepEqual(problems, ['v9.9.9 already exists on abc1234, which is not this commit']);
});

test('a version that is not pinned semver is refused before anything else', () => {
  assert.deepEqual(collectProblems({ ...clean, version: '9.9' }), ['version 9.9 is not pinned semver']);
  assert.ok(SEMVER.test('0.2.0'));
  assert.ok(SEMVER.test('1.0.0-rc.1'));
  assert.ok(!SEMVER.test('1.0'));
});

test('argument parsing rejects a flag with no value', () => {
  assert.deepEqual(parseArguments(['--version', '0.2.0']), { version: '0.2.0' });
  assert.throws(() => parseArguments(['--version']), /--version requires a value/);
  assert.throws(() => parseArguments(['nope']), /unexpected argument nope/);
});

test('the manifests are read as they are written', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-preflight-'));
  try {
    fs.mkdirSync(path.join(root, 'apps/cli'), { recursive: true });
    fs.writeFileSync(
      path.join(root, 'apps/cli/package.json'),
      JSON.stringify({ version: '1.2.3', optionalDependencies: { '@orchester/cli-win32-x64': '1.2.3' } }),
      'utf8',
    );
    assert.equal(manifestVersion(root, 'apps/cli/package.json'), '1.2.3');
    assert.deepEqual(cliPlatformPins(root), { '@orchester/cli-win32-x64': '1.2.3' });

    // CRLF is what git writes on Windows, and the Cargo pattern has to survive it.
    fs.mkdirSync(path.join(root, 'apps/desktop/src-tauri'), { recursive: true });
    fs.writeFileSync(
      path.join(root, 'apps/desktop/src-tauri/Cargo.toml'),
      '[package]\r\nname = "orchester-desktop"\r\nversion = "4.5.6"\r\n',
      'utf8',
    );
    assert.equal(manifestVersion(root, 'apps/desktop/src-tauri/Cargo.toml'), '4.5.6');

    fs.mkdirSync(path.join(root, 'npm/plugins/codex'), { recursive: true });
    fs.writeFileSync(path.join(root, 'npm/plugins/codex/package.json'), JSON.stringify({ version: '7.8.9' }), 'utf8');
    fs.writeFileSync(
      path.join(root, 'npm/plugins/codex/orchester-plugin.json'),
      JSON.stringify({ version: '7.8.9' }),
      'utf8',
    );
    assert.deepEqual(pluginVersions(root), [{ directory: 'codex', package: '7.8.9', manifest: '7.8.9' }]);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});