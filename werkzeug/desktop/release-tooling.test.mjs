import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { installerName, assertInputs } from './build-installer.mjs';
import { expectedAssets, parseArguments as parseStageArguments, sha256 } from './stage-release.mjs';
import { expectedAssets as expectedReleaseAssets, parseChecksums, parseArguments as parseVerifyArguments } from './verify-release.mjs';

const repositoryRoot = path.resolve(path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')), '../..');

test('installer naming follows the tauri product architecture convention', () => {
  assert.equal(installerName('0.1.2', 'x64'), 'Orchester_0.1.2_x64-setup.exe');
  assert.equal(installerName('1.0.0', 'arm64'), 'Orchester_1.0.0_arm64-setup.exe');
});

test('release staging and publishing agree on the same asset names', () => {
  assert.deepEqual(expectedAssets('0.1.2'), ['Orchester_0.1.2_x64-setup.exe', 'Orchester_0.1.2_arm64-setup.exe']);
  assert.deepEqual(expectedReleaseAssets('0.1.2'), [
    'Orchester_0.1.2_x64-setup.exe',
    'Orchester_0.1.2_arm64-setup.exe',
    'SHA256SUMS',
  ]);
});

test('argument parsing rejects missing and repeating values', () => {
  assert.deepEqual(parseStageArguments(['--installers', 'a', '--output', 'b', '--version', '0.1.2']), {
    installers: 'a',
    output: 'b',
    version: '0.1.2',
  });
  assert.deepEqual(parseVerifyArguments(['--tag', 'desktop-v0.1.2', '--version', '0.1.2']), {
    tag: 'desktop-v0.1.2',
    version: '0.1.2',
  });
  assert.throws(() => parseStageArguments(['--installers']));
  assert.throws(() => parseStageArguments(['installers']));
});

test('checksum parsing round-trips the staged digest and rejects malformed lines', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-stage-'));
  try {
    const asset = path.join(directory, 'Orchester_0.1.2_x64-setup.exe');
    fs.writeFileSync(asset, 'installer bytes');
    const digest = sha256(asset);
    assert.equal(digest, crypto.createHash('sha256').update('installer bytes').digest('hex'));
    const entries = parseChecksums(`${digest}  Orchester_0.1.2_x64-setup.exe\n`);
    assert.equal(entries.get('Orchester_0.1.2_x64-setup.exe'), digest);
    assert.throws(() => parseChecksums('not-a-checksum\n'));
  } finally {
    fs.rmSync(directory, { recursive: true, force: true });
  }
});

test('the desktop version is pinned consistently across the release manifests', () => {
  const config = JSON.parse(fs.readFileSync(path.join(repositoryRoot, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'));
  const manifest = JSON.parse(fs.readFileSync(path.join(repositoryRoot, 'apps/desktop/package.json'), 'utf8'));
  const cargo = fs.readFileSync(path.join(repositoryRoot, 'apps/desktop/src-tauri/Cargo.toml'), 'utf8');
  const cargoVersion = /^version = "(.+)"$/m.exec(cargo)?.[1];
  assert.equal(config.version, manifest.version);
  assert.equal(config.version, cargoVersion);
  assert.doesNotThrow(() => assertInputs(config.version));
});
