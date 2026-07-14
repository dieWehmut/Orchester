import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import { stagePlatformPackages as stagePlatformPackagesImpl } from './stage.mjs';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');
const metaPackage = JSON.parse(
  fs.readFileSync(path.join(repositoryRoot, 'npm/cli/package.json'), 'utf8'),
);
const targets = JSON.parse(
  fs.readFileSync(path.join(repositoryRoot, 'npm/cli/targets.json'), 'utf8'),
);

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

function testRuntime(overrides = {}) {
  return {
    platform: 'linux',
    modeOf: (file) => (process.platform === 'win32' ? 0o755 : fs.statSync(file).mode),
    ...overrides,
  };
}

function stagePlatformPackages(options) {
  return stagePlatformPackagesImpl({
    ...options,
    runtime: options.runtime ?? testRuntime(),
  });
}

function createFixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'orchester-npm-stage-'));
  t.after(() => fs.rmSync(root, { force: true, recursive: true }));

  const artifactRoot = path.join(root, 'artifacts');
  const outputRoot = path.join(root, 'packages');
  for (const target of targets) {
    const binary = path.join(
      artifactRoot,
      target.rustTarget,
      'release',
      path.basename(target.binaryPath),
    );
    fs.mkdirSync(path.dirname(binary), { recursive: true });
    fs.writeFileSync(binary, nativeHeader(target));
    if (process.platform !== 'win32') fs.chmodSync(binary, 0o755);
  }

  return { artifactRoot, outputRoot, root, runtime: testRuntime() };
}

function stageFixture(fixture, version = metaPackage.version) {
  return stagePlatformPackages({
    artifactRoot: fixture.artifactRoot,
    outputRoot: fixture.outputRoot,
    version,
    runtime: fixture.runtime,
  });
}

test('stages the six exact native packages without package executables or lifecycle scripts', (t) => {
  const fixture = createFixture(t);
  const { artifactRoot, outputRoot } = fixture;

  const staged = stageFixture(fixture);

  assert.equal(staged.length, 6);
  assert.deepEqual(
    staged.map(({ packageName }) => packageName),
    targets.map(({ packageName }) => packageName),
  );

  for (const [index, target] of targets.entries()) {
    const packageRoot = path.join(outputRoot, target.packageName.split('/').at(-1));
    const manifest = JSON.parse(
      fs.readFileSync(path.join(packageRoot, 'package.json'), 'utf8'),
    );

    assert.equal(staged[index].packageRoot, packageRoot);
    assert.deepEqual(manifest, {
      name: target.packageName,
      version: metaPackage.version,
      description: `Orchester native executable for ${target.platform}/${target.arch}`,
      license: metaPackage.license,
      repository: 'https://github.com/dieWehmut/Orchester',
      os: [target.platform],
      cpu: [target.arch],
      files: ['bin'],
      engines: metaPackage.engines,
      publishConfig: { access: 'public' },
    });
    assert.equal(Object.hasOwn(manifest, 'bin'), false);
    for (const lifecycle of ['preinstall', 'install', 'postinstall']) {
      assert.equal(Object.hasOwn(manifest.scripts ?? {}, lifecycle), false);
    }

    const stagedBinary = path.join(packageRoot, target.binaryPath);
    assert.equal(
      Buffer.compare(fs.readFileSync(stagedBinary), nativeHeader(target)),
      0,
    );
  }
});

test('rejects a missing artifact root with a stable typed error', (t) => {
  const { outputRoot, root } = createFixture(t);

  assert.throws(
    () => stagePlatformPackages({
      artifactRoot: path.join(root, 'does-not-exist'),
      outputRoot,
      version: metaPackage.version,
    }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_ARTIFACT_ROOT_MISSING');
      assert.equal(error.message, 'artifact root must be an existing directory');
      return true;
    },
  );
});

test('refuses to stage into an existing output root', (t) => {
  const { artifactRoot, outputRoot } = createFixture(t);
  fs.mkdirSync(outputRoot);
  fs.writeFileSync(path.join(outputRoot, 'stale.txt'), 'must remain untouched');

  assert.throws(
    () => stagePlatformPackages({ artifactRoot, outputRoot, version: metaPackage.version }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_OUTPUT_EXISTS');
      assert.equal(error.message, 'output root must not exist');
      return true;
    },
  );
  assert.equal(fs.readFileSync(path.join(outputRoot, 'stale.txt'), 'utf8'), 'must remain untouched');
});

test('requires a POSIX host and leaves no output on Windows', (t) => {
  const fixture = createFixture(t);

  assert.throws(
    () => stagePlatformPackages({
      artifactRoot: fixture.artifactRoot,
      outputRoot: fixture.outputRoot,
      version: metaPackage.version,
      runtime: { ...fixture.runtime, platform: 'win32' },
    }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_POSIX_HOST_REQUIRED');
      assert.equal(error.message, 'native npm staging must run on a POSIX host');
      return true;
    },
  );
  assert.equal(fs.existsSync(fixture.outputRoot), false);
});

test('refuses a dangling output-root symlink without touching its target', { skip: process.platform === 'win32' }, (t) => {
  const { artifactRoot, outputRoot, root } = createFixture(t);
  const destination = path.join(root, 'missing-output-target');
  fs.symlinkSync(destination, outputRoot, 'junction');

  assert.throws(
    () => stagePlatformPackages({ artifactRoot, outputRoot, version: metaPackage.version }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_OUTPUT_EXISTS');
      assert.equal(error.message, 'output root path is occupied');
      return true;
    },
  );
  assert.equal(fs.existsSync(destination), false);
});

test('rejects a version that is not pinned by the meta package', (t) => {
  const { artifactRoot, outputRoot } = createFixture(t);

  assert.throws(
    () => stagePlatformPackages({ artifactRoot, outputRoot, version: '9.9.9' }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_VERSION_MISMATCH');
      assert.equal(error.message, 'staging version must match the CLI package version');
      return true;
    },
  );
});

test('rejects a missing native binary before creating any package directory', (t) => {
  const { artifactRoot, outputRoot } = createFixture(t);
  const missingTarget = targets[2];
  fs.rmSync(
    path.join(artifactRoot, missingTarget.rustTarget, 'release', path.basename(missingTarget.binaryPath)),
  );

  assert.throws(
    () => stagePlatformPackages({ artifactRoot, outputRoot, version: metaPackage.version }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_BINARY_MISSING');
      assert.equal(error.message, `native executable is missing for ${missingTarget.rustTarget}`);
      return true;
    },
  );
  assert.equal(fs.existsSync(outputRoot), false);
});

test('rejects a directory in place of a native binary', (t) => {
  const { artifactRoot, outputRoot } = createFixture(t);
  const invalidTarget = targets[0];
  const binary = path.join(
    artifactRoot,
    invalidTarget.rustTarget,
    'release',
    path.basename(invalidTarget.binaryPath),
  );
  fs.rmSync(binary);
  fs.mkdirSync(binary);

  assert.throws(
    () => stagePlatformPackages({ artifactRoot, outputRoot, version: metaPackage.version }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_BINARY_NOT_FILE');
      return true;
    },
  );
  assert.equal(fs.existsSync(outputRoot), false);
});

test('rejects a symlinked native binary that points outside the artifact root', { skip: process.platform === 'win32' }, (t) => {
  const { artifactRoot, outputRoot, root } = createFixture(t);
  const symlinkTarget = targets[1];
  const binary = path.join(
    artifactRoot,
    symlinkTarget.rustTarget,
    'release',
    path.basename(symlinkTarget.binaryPath),
  );
  const outside = path.join(root, 'outside-native');
  fs.writeFileSync(outside, 'outside');
  fs.rmSync(binary);
  fs.symlinkSync(outside, binary);

  assert.throws(
    () => stagePlatformPackages({ artifactRoot, outputRoot, version: metaPackage.version }),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_BINARY_NOT_FILE');
      return true;
    },
  );
  assert.equal(fs.existsSync(outputRoot), false);
});

test('rejects a non-executable POSIX native binary', (t) => {
  const fixture = createFixture(t);
  const { artifactRoot, outputRoot } = fixture;
  const nonExecutableTarget = targets[0];
  const binary = path.join(
    artifactRoot,
    nonExecutableTarget.rustTarget,
    'release',
    path.basename(nonExecutableTarget.binaryPath),
  );
  if (process.platform !== 'win32') fs.chmodSync(binary, 0o644);
  fixture.runtime.modeOf = () => 0o644;

  assert.throws(
    () => stageFixture(fixture),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_BINARY_NOT_EXECUTABLE');
      return true;
    },
  );
  assert.equal(fs.existsSync(outputRoot), false);
});

test('cleans its private sibling temporary directory after a copy failure', (t) => {
  const fixture = createFixture(t);
  let copies = 0;
  fixture.runtime.copyFile = (source, destination) => {
    copies += 1;
    if (copies === 3) throw new Error('injected copy failure');
    fs.copyFileSync(source, destination);
  };

  assert.throws(() => stageFixture(fixture), /injected copy failure/);
  assert.equal(fs.existsSync(fixture.outputRoot), false);
  assert.deepEqual(
    fs.readdirSync(fixture.root).filter((name) => name.startsWith('packages.tmp-')),
    [],
  );
});

test('rejects a valid native header for the wrong architecture before staging', (t) => {
  const fixture = createFixture(t);
  const wrongTarget = targets[0];
  const binary = path.join(
    fixture.artifactRoot,
    wrongTarget.rustTarget,
    'release',
    path.basename(wrongTarget.binaryPath),
  );
  fs.writeFileSync(binary, nativeHeader({ ...wrongTarget, arch: 'arm64' }));

  assert.throws(
    () => stageFixture(fixture),
    (error) => {
      assert.equal(error.code, 'ORCHESTER_NPM_BINARY_WRONG_ARCH');
      assert.equal(error.message, `native executable architecture does not match ${wrongTarget.rustTarget}`);
      return true;
    },
  );
  assert.equal(fs.existsSync(fixture.outputRoot), false);
});

test('command line entry point stages packages with explicit roots and version', { skip: process.platform === 'win32' }, (t) => {
  const { artifactRoot, outputRoot } = createFixture(t);
  const result = spawnSync(process.execPath, [
    path.join(moduleDirectory, 'stage.mjs'),
    '--artifacts',
    artifactRoot,
    '--output',
    outputRoot,
    '--version',
    metaPackage.version,
  ], {
    encoding: 'utf8',
    timeout: 10_000,
  });

  assert.equal(result.error, undefined);
  assert.equal(result.status, 0);
  assert.equal(result.stderr, '');
  assert.equal(result.stdout, `Staged 6 native npm packages in ${outputRoot}\n`);
  assert.equal(fs.existsSync(path.join(outputRoot, 'cli-linux-x64/package.json')), true);
});

test('command line entry point fails closed on a non-POSIX host', (t) => {
  const { artifactRoot, outputRoot } = createFixture(t);
  const result = spawnSync(process.execPath, [
    path.join(moduleDirectory, 'stage.mjs'),
    '--artifacts',
    artifactRoot,
    '--output',
    outputRoot,
    '--version',
    metaPackage.version,
  ], {
    encoding: 'utf8',
    timeout: 10_000,
  });

  if (process.platform !== 'win32') {
    assert.equal(result.status, 0);
    return;
  }
  assert.equal(result.status, 1);
  assert.equal(result.stdout, '');
  assert.equal(result.stderr, 'native npm staging must run on a POSIX host\n');
  assert.doesNotMatch(result.stderr, /(?:\n\s+at\s|node:internal|Error:)/);
  assert.equal(fs.existsSync(outputRoot), false);
});
