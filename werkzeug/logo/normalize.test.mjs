import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { normalizeChafaOutput } from './normalize.mjs';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

function utf16leWithBom(text) {
  return Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from(text, 'utf16le')]);
}

test('normalizes a PowerShell UTF-16 Chafa redirect without losing true colour', () => {
  const input = utf16leWithBom(
    '\x1b[?25l\x1b[38;2;1;2;3m\u923b\u20ac \x1b[0m\r\n'
      + '\x1b[48;2;4;5;6m \u923b\u20ac\x1b[0m\r\n\x1b[?25h\r\n',
  );

  const normalized = normalizeChafaOutput(input, { expectedRows: 2, maxColumns: 2 });

  assert.equal(
    normalized,
    '\x1b[38;2;1;2;3m\u2580 \x1b[0m\n\x1b[48;2;4;5;6m \u2580\x1b[0m\n',
  );
  assert.equal(normalized.includes('\x1b[?25'), false);
});

test('rejects non-SGR terminal controls and invalid source geometry', () => {
  assert.throws(
    () => normalizeChafaOutput(Buffer.from('\x1b[2J \n'), {
      expectedRows: 1,
      maxColumns: 1,
    }),
    (error) => error.code === 'ORCHESTER_LOGO_UNSAFE_ANSI',
  );
  assert.throws(
    () => normalizeChafaOutput(Buffer.from('   \n'), {
      expectedRows: 1,
      maxColumns: 2,
    }),
    (error) => error.code === 'ORCHESTER_LOGO_GEOMETRY',
  );
});

test('the packaged logo is a normalized 66x30 Chafa vhalf source', () => {
  const asset = fs.readFileSync(path.join(repositoryRoot, 'kisten/konsole/assets/logo.ansi'));
  const normalized = normalizeChafaOutput(asset, { expectedRows: 30, maxColumns: 66 });

  assert.equal(normalized, asset.toString('utf8'));
  assert.ok(Array.from(normalized).some((character) => character.codePointAt(0) === 0x2580));
});
