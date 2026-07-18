import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { verifyAgentPluginPackage } from './plugin.mjs';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');
const packageRoot = path.join(repositoryRoot, 'npm/plugins/claude');
const canonicalManifest = path.join(repositoryRoot, 'manifeste/claude.toml');

function repositoryPackage(name) {
  return verifyAgentPluginPackage({
    canonicalManifest: path.join(repositoryRoot, `manifeste/${name}.toml`),
    expectedName: `@orchester/${name}`,
    expectedVersion: '0.1.0',
    packageRoot: path.join(repositoryRoot, `npm/plugins/${name}`),
  });
}

function fixture(mutator) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-plugin-package-'));
  const candidate = path.join(root, 'claude');
  fs.cpSync(packageRoot, candidate, { recursive: true });
  mutator?.(candidate);
  return { candidate, root };
}

function withFixture(mutator, callback) {
  const { candidate, root } = fixture(mutator);
  try {
    callback(candidate);
  } finally {
    fs.rmSync(root, { force: true, recursive: true });
  }
}

function rewriteJson(file, mutate) {
  const value = JSON.parse(fs.readFileSync(file, 'utf8'));
  mutate(value);
  fs.writeFileSync(file, `${JSON.stringify(value, null, 2)}\n`);
}

function assertCode(expected, callback) {
  assert.throws(callback, (error) => error?.code === expected);
}

test('repository Claude package satisfies the locked pure-data contract', () => {
  const verified = verifyAgentPluginPackage({
    canonicalManifest,
    expectedName: '@orchester/claude',
    expectedVersion: '0.1.0',
    packageRoot,
  });

  assert.deepEqual(verified, {
    adapterManifest: 'manifests/claude.toml',
    command: 'claude',
    name: 'claude',
    packageName: '@orchester/claude',
    version: '0.1.0',
  });
});

test('repository Codex package satisfies the locked pure-data contract', () => {
  assert.deepEqual(repositoryPackage('codex'), {
    adapterManifest: 'manifests/codex.toml',
    command: 'codex',
    name: 'codex',
    packageName: '@orchester/codex',
    version: '0.1.0',
  });
});

test('lifecycle scripts, executable bins, and dependency graphs are rejected', () => {
  for (const [field, value] of [
    ['scripts', { postinstall: 'node install.js' }],
    ['bin', { claude: 'install.js' }],
    ['dependencies', { package: '1.0.0' }],
    ['optionalDependencies', { package: '1.0.0' }],
    ['peerDependencies', { package: '1.0.0' }],
    ['devDependencies', { package: '1.0.0' }],
  ]) {
    withFixture(
      (candidate) => rewriteJson(path.join(candidate, 'package.json'), (manifest) => {
        manifest[field] = value;
      }),
      (candidate) => assertCode('ORCHESTER_PLUGIN_EXECUTABLE_PACKAGE', () => {
        verifyAgentPluginPackage({
          canonicalManifest,
          expectedName: '@orchester/claude',
          expectedVersion: '0.1.0',
          packageRoot: candidate,
        });
      }),
    );
  }
});

test('descriptor identity, schema, and adapter path fail closed', () => {
  for (const mutate of [
    (descriptor) => { descriptor.schemaVersion = 2; },
    (descriptor) => { descriptor.name = 'codex'; },
    (descriptor) => { descriptor.packageName = '@orchester/codex'; },
    (descriptor) => { descriptor.version = '0.1.1'; },
    (descriptor) => { descriptor.adapterManifest = '../claude.toml'; },
    (descriptor) => { descriptor.command = 'claude --dangerous'; },
  ]) {
    withFixture(
      (candidate) => rewriteJson(path.join(candidate, 'orchester-plugin.json'), mutate),
      (candidate) => assertCode('ORCHESTER_PLUGIN_DESCRIPTOR_INVALID', () => {
        verifyAgentPluginPackage({
          canonicalManifest,
          expectedName: '@orchester/claude',
          expectedVersion: '0.1.0',
          packageRoot: candidate,
        });
      }),
    );
  }
});

test('manifest drift and undeclared package members are rejected', () => {
  withFixture(
    (candidate) => fs.appendFileSync(path.join(candidate, 'manifests/claude.toml'), '\nstreaming = false\n'),
    (candidate) => assertCode('ORCHESTER_PLUGIN_MANIFEST_DRIFT', () => {
      verifyAgentPluginPackage({
        canonicalManifest,
        expectedName: '@orchester/claude',
        expectedVersion: '0.1.0',
        packageRoot: candidate,
      });
    }),
  );

  withFixture(
    (candidate) => fs.writeFileSync(path.join(candidate, 'install.js'), 'process.exit(0);\n'),
    (candidate) => assertCode('ORCHESTER_PLUGIN_PACKAGE_MEMBERS', () => {
      verifyAgentPluginPackage({
        canonicalManifest,
        expectedName: '@orchester/claude',
        expectedVersion: '0.1.0',
        packageRoot: candidate,
      });
    }),
  );
});

test('linked package members are rejected when the host permits links', (context) => {
  const { candidate, root } = fixture();
  const manifest = path.join(candidate, 'manifests/claude.toml');
  const target = path.join(root, 'foreign.toml');
  fs.writeFileSync(target, fs.readFileSync(manifest));
  fs.rmSync(manifest);
  try {
    fs.symlinkSync(target, manifest, 'file');
  } catch (error) {
    fs.rmSync(root, { force: true, recursive: true });
    if (['EPERM', 'EACCES', 'ENOTSUP'].includes(error?.code)) {
      context.skip('host denied symlink creation');
      return;
    }
    throw error;
  }

  try {
    assertCode('ORCHESTER_PLUGIN_PACKAGE_MEMBERS', () => {
      verifyAgentPluginPackage({
        canonicalManifest,
        expectedName: '@orchester/claude',
        expectedVersion: '0.1.0',
        packageRoot: candidate,
      });
    });
  } finally {
    fs.rmSync(root, { force: true, recursive: true });
  }
});
