import assert from 'node:assert/strict';
import test from 'node:test';

import { validateNativeHeader } from './binary.mjs';

const targets = [
  { platform: 'linux', arch: 'x64', rustTarget: 'x86_64-unknown-linux-musl' },
  { platform: 'linux', arch: 'arm64', rustTarget: 'aarch64-unknown-linux-musl' },
  { platform: 'darwin', arch: 'x64', rustTarget: 'x86_64-apple-darwin' },
  { platform: 'darwin', arch: 'arm64', rustTarget: 'aarch64-apple-darwin' },
  { platform: 'win32', arch: 'x64', rustTarget: 'x86_64-pc-windows-msvc' },
  { platform: 'win32', arch: 'arm64', rustTarget: 'aarch64-pc-windows-msvc' },
];

function nativeHeader({ platform, arch }) {
  if (platform === 'linux') {
    const header = Buffer.alloc(64);
    header.set([0x7f, 0x45, 0x4c, 0x46, 2, 1]);
    header.writeUInt16LE(arch === 'x64' ? 0x3e : 0xb7, 18);
    return header;
  }
  if (platform === 'darwin') {
    const header = Buffer.alloc(32);
    header.set([0xcf, 0xfa, 0xed, 0xfe]);
    header.writeUInt32LE(arch === 'x64' ? 0x01000007 : 0x0100000c, 4);
    return header;
  }

  const header = Buffer.alloc(128);
  header.set([0x4d, 0x5a]);
  header.writeUInt32LE(0x40, 0x3c);
  header.set([0x50, 0x45, 0, 0], 0x40);
  header.writeUInt16LE(arch === 'x64' ? 0x8664 : 0xaa64, 0x44);
  return header;
}

test('accepts the exact 64-bit executable header for every supported target', () => {
  for (const target of targets) {
    assert.doesNotThrow(() => validateNativeHeader(nativeHeader(target), target));
  }
});

test('rejects text and truncated executable headers', () => {
  for (const target of targets) {
    for (const bytes of [Buffer.from('not a native executable'), Buffer.alloc(4)]) {
      assert.throws(
        () => validateNativeHeader(bytes, target),
        (error) => {
          assert.equal(error.code, 'ORCHESTER_NPM_BINARY_INVALID_FORMAT');
          return true;
        },
      );
    }
  }
});

test('rejects a valid executable header for the wrong architecture', () => {
  for (const target of targets) {
    const otherArch = target.arch === 'x64' ? 'arm64' : 'x64';
    assert.throws(
      () => validateNativeHeader(nativeHeader({ ...target, arch: otherArch }), target),
      (error) => {
        assert.equal(error.code, 'ORCHESTER_NPM_BINARY_WRONG_ARCH');
        assert.equal(error.message, `native executable architecture does not match ${target.rustTarget}`);
        return true;
      },
    );
  }
});

export { nativeHeader };
