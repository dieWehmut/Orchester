import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const SGR = /\x1b\[[0-9;]*m/g;
const CURSOR_VISIBILITY = /\x1b\[\?25[lh]/g;
const HALF_BLOCK_MOJIBAKE = /\u923b\u20ac/g;

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, '../..');
const defaultInput = path.join(repositoryRoot, 'sample/logo/logo.txt');
const defaultOutput = path.join(repositoryRoot, 'kisten/konsole/assets/logo.ansi');

function fail(code, message) {
  const error = new Error(message);
  error.code = code;
  throw error;
}

function decodeRedirect(input) {
  const bytes = Buffer.isBuffer(input) ? input : Buffer.from(input);
  if (bytes[0] === 0xff && bytes[1] === 0xfe) {
    return bytes.subarray(2).toString('utf16le');
  }
  if (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    return bytes.subarray(3).toString('utf8');
  }
  return bytes.toString('utf8');
}

function visibleRow(row) {
  return row.replace(SGR, '');
}

export function normalizeChafaOutput(input, {
  expectedRows = 30,
  maxColumns = 66,
} = {}) {
  if (!Number.isInteger(expectedRows) || expectedRows < 1
    || !Number.isInteger(maxColumns) || maxColumns < 1) {
    fail('ORCHESTER_LOGO_GEOMETRY', 'logo geometry must use positive integer bounds');
  }

  let text = decodeRedirect(input)
    .replace(/^\uFEFF/, '')
    .replace(HALF_BLOCK_MOJIBAKE, '\u2580')
    .replace(CURSOR_VISIBILITY, '')
    .replace(/\r\n?/g, '\n');

  const rows = text.split('\n');
  while (rows.at(-1) === '') rows.pop();
  if (rows.length !== expectedRows) {
    fail(
      'ORCHESTER_LOGO_GEOMETRY',
      `expected ${expectedRows} Chafa rows, found ${rows.length}`,
    );
  }

  let halfBlocks = 0;
  for (const [index, row] of rows.entries()) {
    const visible = visibleRow(row);
    if (visible.includes('\x1b')) {
      fail('ORCHESTER_LOGO_UNSAFE_ANSI', `row ${index + 1} contains a non-SGR escape`);
    }
    const characters = Array.from(visible);
    if (characters.length > maxColumns) {
      fail(
        'ORCHESTER_LOGO_GEOMETRY',
        `row ${index + 1} exceeds the ${maxColumns}-column Chafa canvas`,
      );
    }
    for (const character of characters) {
      if (character === '\u2580') {
        halfBlocks += 1;
      } else if (character !== ' ') {
        fail(
          'ORCHESTER_LOGO_UNSAFE_ANSI',
          `row ${index + 1} contains an unsupported printable character`,
        );
      }
    }
  }
  if (halfBlocks === 0) {
    fail('ORCHESTER_LOGO_GEOMETRY', 'Chafa source contains no vhalf symbols');
  }

  text = `${rows.join('\n')}\n`;
  return text;
}

export function normalizeLogoFile(inputPath = defaultInput, outputPath = defaultOutput) {
  const normalized = normalizeChafaOutput(fs.readFileSync(inputPath));
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, normalized, { encoding: 'utf8' });
  return { inputPath, outputPath, bytes: Buffer.byteLength(normalized) };
}

const isMain = process.argv[1]
  && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url));

if (isMain) {
  const args = process.argv.slice(2);
  if (args.length !== 0 && args.length !== 2) {
    process.stderr.write('usage: node werkzeug/logo/normalize.mjs [input.txt output.ansi]\n');
    process.exitCode = 1;
  } else {
    try {
      const result = normalizeLogoFile(args[0], args[1]);
      process.stdout.write(`normalized ${result.bytes} bytes to ${result.outputPath}\n`);
    } catch (error) {
      const message = typeof error?.code === 'string' && error.code.startsWith('ORCHESTER_LOGO_')
        ? error.message
        : 'failed to normalize the Chafa logo';
      process.stderr.write(`${message}\n`);
      process.exitCode = 1;
    }
  }
}
