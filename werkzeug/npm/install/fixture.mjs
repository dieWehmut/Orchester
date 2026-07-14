import fs from 'node:fs';
import path from 'node:path';
import { gzipSync } from 'node:zlib';

import { ensureDirectory } from './environment.mjs';

export const FIXTURE_NAME = 'orchester-install-smoke-fixture';
export const FIXTURE_VERSION = '1.0.0';
export const FIXTURE_MARKER = 'ORCHESTER_INSTALL_SMOKE_OK';

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function writeTarField(header, offset, length, value) {
  const bytes = Buffer.from(String(value), 'utf8');
  if (bytes.length > length) {
    fail('ORCHESTER_NPM_FIXTURE_INVALID', `tar field is too long at offset ${offset}`);
  }
  bytes.copy(header, offset);
}

function writeTarOctal(header, offset, length, value) {
  const encoded = Number(value).toString(8).padStart(length - 1, '0');
  if (encoded.length > length - 1) {
    fail('ORCHESTER_NPM_FIXTURE_INVALID', 'tar numeric field is too large');
  }
  writeTarField(header, offset, length, `${encoded}\0`);
}

function tarMember(name, body, mode) {
  const contents = Buffer.from(body, 'utf8');
  const header = Buffer.alloc(512);
  writeTarField(header, 0, 100, name);
  writeTarOctal(header, 100, 8, mode);
  writeTarOctal(header, 108, 8, 0);
  writeTarOctal(header, 116, 8, 0);
  writeTarOctal(header, 124, 12, contents.length);
  writeTarOctal(header, 136, 12, 0);
  header.fill(0x20, 148, 156);
  writeTarField(header, 156, 1, '0');
  writeTarField(header, 257, 6, 'ustar\0');
  writeTarField(header, 263, 2, '00');
  writeTarField(header, 265, 32, 'orchester');
  writeTarField(header, 297, 32, 'orchester');
  let checksum = 0;
  for (const byte of header) checksum += byte;
  writeTarField(header, 148, 8, `${checksum.toString(8).padStart(6, '0')}\0 `);
  const padding = Buffer.alloc((512 - (contents.length % 512)) % 512);
  return Buffer.concat([header, contents, padding]);
}

/** Create a tiny dependency-free package without invoking a registry or packer. */
export function createFixtureTarball(root) {
  const resolvedRoot = path.resolve(root);
  ensureDirectory(resolvedRoot);
  const tarball = path.join(resolvedRoot, `${FIXTURE_NAME}-${FIXTURE_VERSION}.tgz`);
  const manifest = `${JSON.stringify({
    name: FIXTURE_NAME,
    version: FIXTURE_VERSION,
    description: 'Orchester package-manager isolation fixture',
    bin: { [FIXTURE_NAME]: 'bin/fixture.cjs' },
    files: ['bin'],
  }, null, 2)}\n`;
  const executable = `#!/usr/bin/env node\nprocess.stdout.write(${JSON.stringify(`${FIXTURE_MARKER}\n`)});\n`;
  const archive = Buffer.concat([
    tarMember('package/package.json', manifest, 0o644),
    tarMember('package/bin/fixture.cjs', executable, 0o755),
    Buffer.alloc(1024),
  ]);
  fs.writeFileSync(tarball, gzipSync(archive), { flag: 'wx', mode: 0o600 });
  return tarball;
}
