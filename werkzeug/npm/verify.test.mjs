import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  parseCommandLine,
  parseTarArchive,
  verifyNpmPackages,
} from './verify.mjs';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');
const metaManifest = JSON.parse(
  fs.readFileSync(path.join(repositoryRoot, 'npm/cli/package.json'), 'utf8'),
);

function writeJson(file, value) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function makePackage(root, manifest, files = {}) {
  fs.mkdirSync(root, { recursive: true });
  writeJson(path.join(root, 'package.json'), manifest);
  for (const [relative, contents] of Object.entries(files)) {
    const file = path.join(root, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, contents);
  }
}

function createFixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-npm-verify-'));
  t.after(() => fs.rmSync(root, { force: true, recursive: true }));

  const meta = path.join(root, 'meta');
  makePackage(meta, {
    name: metaManifest.name,
    version: metaManifest.version,
    bin: { orchester: 'bin/orchester.cjs' },
    files: ['bin', 'lib', 'targets.json'],
    optionalDependencies: {
      '@orchester/cli-linux-x64': metaManifest.version,
    },
  }, {
    'bin/orchester.cjs': '#!/usr/bin/env node\n',
    'lib/process.cjs': 'module.exports = {};\n',
    'lib/target.cjs': 'module.exports = {};\n',
    'targets.json': '[]\n',
  });
  if (process.platform !== 'win32') {
    fs.chmodSync(path.join(meta, 'bin/orchester.cjs'), 0o755);
  }

  const platform = path.join(root, 'platform');
  makePackage(platform, {
    name: '@orchester/cli-linux-x64',
    version: metaManifest.version,
    files: ['bin'],
  }, {
    'bin/orchester': '#!/bin/sh\nexit 0\n',
  });
  if (process.platform !== 'win32') {
    fs.chmodSync(path.join(platform, 'bin/orchester'), 0o755);
  }

  return { meta, platform, root };
}

function writeTarText(header, offset, length, value) {
  const bytes = Buffer.from(value, 'utf8');
  assert.ok(bytes.length <= length);
  bytes.copy(header, offset);
}

function writeTarOctal(header, offset, length, value) {
  const encoded = value.toString(8).padStart(length - 1, '0');
  assert.equal(encoded.length, length - 1);
  writeTarText(header, offset, length, `${encoded}\0`);
}

function tarMember({
  body = '',
  gid = 0,
  magic = 'ustar\0',
  mode = 0o644,
  name,
  type = '0',
  uid = 0,
  version = '00',
}) {
  const contents = Buffer.from(body);
  const header = Buffer.alloc(512);
  writeTarText(header, 0, 100, name);
  writeTarOctal(header, 100, 8, mode);
  writeTarOctal(header, 108, 8, uid);
  writeTarOctal(header, 116, 8, gid);
  writeTarOctal(header, 124, 12, contents.length);
  writeTarOctal(header, 136, 12, 0);
  header.fill(0x20, 148, 156);
  writeTarText(header, 156, 1, type);
  writeTarText(header, 257, 6, magic);
  writeTarText(header, 263, 2, version);
  let checksum = 0;
  for (const byte of header) checksum += byte;
  const checksumText = checksum.toString(8).padStart(6, '0');
  writeTarText(header, 148, 8, `${checksumText}\0 `);
  const padding = Buffer.alloc((512 - (contents.length % 512)) % 512);
  return Buffer.concat([header, contents, padding]);
}

function tarArchive(...members) {
  return Buffer.concat([...members, Buffer.alloc(1024)]);
}

test('rejects verification on Windows before touching package directories', () => {
  if (process.platform !== 'win32') return;
  assert.throws(
    () => verifyNpmPackages({ meta: 'missing-meta', platformDirs: ['missing-platform'] }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_POSIX_HOST_REQUIRED');
      assert.equal(error.message, 'npm package verification must run on a POSIX host');
      return true;
    },
  );
});

test('verifies the exact meta and platform package contents with real npm pack', { skip: process.platform === 'win32' }, (t) => {
  const fixture = createFixture(t);
  const result = verifyNpmPackages({
    meta: fixture.meta,
    platformDirs: [fixture.platform],
  });

  assert.equal(result.length, 2);
  assert.deepEqual(result.map(({ name, version }) => ({ name, version })), [
    { name: metaManifest.name, version: metaManifest.version },
    { name: '@orchester/cli-linux-x64', version: metaManifest.version },
  ]);
  assert.equal(
    fs.readdirSync(fixture.root).some((name) => name.startsWith('.orchester-npm-verify-')),
    false,
  );
});

test('rejects an extra meta member and cleans temporary packs', { skip: process.platform === 'win32' }, (t) => {
  const fixture = createFixture(t);
  fs.writeFileSync(path.join(fixture.meta, 'lib/unexpected.txt'), 'must not ship');

  assert.throws(
    () => verifyNpmPackages({ meta: fixture.meta, platformDirs: [fixture.platform] }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_PACKAGE_EXTRA_MEMBER');
      assert.match(error.message, /lib\/unexpected\.txt/);
      return true;
    },
  );
  assert.equal(
    fs.readdirSync(fixture.root).some((name) => name.startsWith('.orchester-npm-verify-')),
    false,
  );
});

test('rejects a non-executable native member even when npm reports mode 0644', { skip: process.platform === 'win32' }, (t) => {
  const fixture = createFixture(t);
  fs.chmodSync(path.join(fixture.platform, 'bin/orchester'), 0o644);

  assert.throws(
    () => verifyNpmPackages({ meta: fixture.meta, platformDirs: [fixture.platform] }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_PACKAGE_MODE');
      assert.match(error.message, /bin\/orchester/);
      return true;
    },
  );
});

test('rejects package manifest name and version mismatches', { skip: process.platform === 'win32' }, (t) => {
  const fixture = createFixture(t);
  const manifestFile = path.join(fixture.platform, 'package.json');
  const manifest = JSON.parse(fs.readFileSync(manifestFile, 'utf8'));
  manifest.version = '9.9.9';
  writeJson(manifestFile, manifest);

  assert.throws(
    () => verifyNpmPackages({ meta: fixture.meta, platformDirs: [fixture.platform] }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_PACKAGE_VERSION_MISMATCH');
      return true;
    },
  );
});

test('rejects unsafe tar members, links, duplicate paths, non-root owners, and malformed headers', { skip: process.platform === 'win32' }, () => {
  const cases = [
    {
      code: 'ORCHESTER_NPM_ARCHIVE_INVALID',
      archive: tarArchive(tarMember({ body: 'x', name: 'package/../outside' })),
    },
    {
      code: 'ORCHESTER_NPM_PACKAGE_SYMLINK',
      archive: tarArchive(tarMember({ name: 'package/bin/orchester', type: '2' })),
    },
    {
      code: 'ORCHESTER_NPM_ARCHIVE_INVALID',
      archive: tarArchive(
        tarMember({ body: 'x', name: 'package/a' }),
        tarMember({ body: 'y', name: 'package/a' }),
      ),
    },
    {
      code: 'ORCHESTER_NPM_ARCHIVE_INVALID',
      archive: tarArchive(tarMember({ body: 'x', gid: 1, name: 'package/a' })),
    },
    {
      code: 'ORCHESTER_NPM_ARCHIVE_INVALID',
      archive: tarArchive(tarMember({ body: 'x', magic: 'bad\0\0\0', name: 'package/a' })),
    },
  ];

  for (const { archive, code } of cases) {
    assert.throws(
      () => parseTarArchive(archive),
      (error) => {
        assert.equal(error.code, code);
        return true;
      },
    );
  }
});

test('returns tar member modes for native mode enforcement', { skip: process.platform === 'win32' }, () => {
  const entries = parseTarArchive(tarArchive(
    tarMember({ body: 'x', mode: 0o755, name: 'package/bin/orchester' }),
  ));
  assert.deepEqual(entries.map(({ path: member, mode }) => ({ member, mode })), [
    { member: 'bin/orchester', mode: 0o755 },
  ]);
});

test('bounds the number of tar members', { skip: process.platform === 'win32' }, () => {
  const archive = tarArchive(...Array.from({ length: 65 }, (_, index) => (
    tarMember({ body: 'x', name: `package/member-${index}` })
  )));
  assert.throws(
    () => parseTarArchive(archive),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_ARCHIVE_INVALID');
      assert.match(error.message, /too many/);
      return true;
    },
  );
});

test('parses repeated command-line platform directories', () => {
  assert.deepEqual(
    parseCommandLine(['--meta', 'meta', '--platform-dir', 'one', '--platform-dir', 'two']),
    { meta: 'meta', platformDirs: ['one', 'two'] },
  );
});
