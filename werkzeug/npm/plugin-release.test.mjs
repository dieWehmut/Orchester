import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  OFFICIAL_AGENT_PLUGINS,
  verifyOfficialAgentPlugins,
} from './plugin-release.mjs';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');

function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-plugin-release-'));
  t.after(() => fs.rmSync(root, { force: true, recursive: true }));
  fs.mkdirSync(path.join(root, 'npm/cli'), { recursive: true });
  fs.mkdirSync(path.join(root, 'manifeste'), { recursive: true });
  fs.copyFileSync(
    path.join(repositoryRoot, 'npm/cli/package.json'),
    path.join(root, 'npm/cli/package.json'),
  );
  for (const name of OFFICIAL_AGENT_PLUGINS) {
    fs.cpSync(
      path.join(repositoryRoot, `npm/plugins/${name}`),
      path.join(root, `npm/plugins/${name}`),
      { recursive: true },
    );
    fs.copyFileSync(
      path.join(repositoryRoot, `manifeste/${name}.toml`),
      path.join(root, `manifeste/${name}.toml`),
    );
  }
  return root;
}

test('repository exposes the exact version-locked official plugin matrix', () => {
  const verified = verifyOfficialAgentPlugins({
    expectedVersion: '0.1.0',
    repositoryRoot,
  });

  assert.deepEqual(verified.map(({ name, packageName, version }) => ({
    name,
    packageName,
    version,
  })), [
    { name: 'claude', packageName: '@orchester/claude', version: '0.1.0' },
    { name: 'codex', packageName: '@orchester/codex', version: '0.1.0' },
    { name: 'opencode', packageName: '@orchester/opencode', version: '0.1.0' },
  ]);
});

test('release matrix rejects missing and unexpected plugin directories', (t) => {
  const root = fixture(t);
  fs.rmSync(path.join(root, 'npm/plugins/codex'), { recursive: true });
  fs.mkdirSync(path.join(root, 'npm/plugins/foreign'));

  assert.throws(
    () => verifyOfficialAgentPlugins({ expectedVersion: '0.1.0', repositoryRoot: root }),
    (error) => error?.code === 'ORCHESTER_PLUGIN_RELEASE_MATRIX',
  );
});

test('release matrix version must match the CLI package', (t) => {
  const root = fixture(t);

  assert.throws(
    () => verifyOfficialAgentPlugins({ expectedVersion: '0.1.1', repositoryRoot: root }),
    (error) => error?.code === 'ORCHESTER_PLUGIN_RELEASE_VERSION',
  );
});

test('command line verifies the repository plugin matrix', () => {
  const result = spawnSync(process.execPath, [
    path.join(moduleDirectory, 'plugin-release.mjs'),
    '--version',
    '0.1.0',
  ], { encoding: 'utf8', timeout: 10_000 });

  assert.equal(result.status, 0);
  assert.equal(result.stderr, '');
  assert.equal(result.stdout, 'Verified 3 official agent plugin packages\n');
});
