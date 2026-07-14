import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const workflowPath = path.join(repositoryRoot, '.github/workflows/npm-release.yml');
const targets = JSON.parse(
  fs.readFileSync(path.join(repositoryRoot, 'npm/cli/targets.json'), 'utf8'),
);

test('release workflow builds every native target before staging packages', () => {
  const workflow = fs.readFileSync(workflowPath, 'utf8');

  for (const target of targets) {
    assert.match(workflow, new RegExp(`rust_target: ${target.rustTarget}`));
    assert.match(workflow, new RegExp(`package: ${target.packageName.split('/').at(-1)}`));
  }
  const runnerByPackage = {
    'cli-linux-x64': 'ubuntu-24.04',
    'cli-linux-arm64': 'ubuntu-24.04-arm',
    'cli-darwin-x64': 'macos-15-intel',
    'cli-darwin-arm64': 'macos-15',
    'cli-win32-x64': 'windows-2025',
    'cli-win32-arm64': 'windows-11-arm',
  };
  for (const target of targets) {
    const packageName = target.packageName.split('/').at(-1);
    const block = new RegExp(
      `package: ${packageName}\\n\\s+runner: ${runnerByPackage[packageName]}\\n\\s+family: ${target.platform === 'win32' ? 'windows' : 'posix'}\\n\\s+rust_target: ${target.rustTarget}\\n\\s+binary: ${target.binaryPath.split('/').at(-1)}`,
    );
    assert.match(workflow, block);
  }
  assert.match(workflow, /node werkzeug\/npm\/stage\.mjs/);
  assert.match(workflow, /node werkzeug\/npm\/verify\.mjs/);
  assert.match(workflow, /\n  validate:\n/);
  assert.match(workflow, /\n  test:\n/);
  assert.match(workflow, /\n  build:\n(?:.|\n)*?    needs: validate\n/);
  assert.match(workflow, /\n  stage:\n(?:.|\n)*?    needs: \[build, test\]\n/);
  assert.equal(workflow.match(/ref: \$\{\{ github\.sha \}\}/g)?.length, 4);
  assert.equal(workflow.includes('inputs.ref'), false);
});

test('release workflow uses current first-party actions and preserves native archives', () => {
  const workflow = fs.readFileSync(workflowPath, 'utf8');

  const pinnedActions = [
    ['checkout', '9c091bb21b7c1c1d1991bb908d89e4e9dddfe3e0', 'v7'],
    ['setup-node', '249970729cb0ef3589644e2896645e5dc5ba9c38', 'v6'],
    ['upload-artifact', '043fb46d1a93c77aae656e7c1c64a875d1fc6a0a', 'v7'],
    ['download-artifact', '3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c', 'v8'],
  ];
  for (const [name, sha, version] of pinnedActions) {
    assert.match(workflow, new RegExp(`actions/${name}@${sha} # ${version}`));
    assert.equal(workflow.includes(`actions/${name}@${version}`), false);
  }
  assert.match(workflow, /archive: false/);
  assert.match(workflow, /if-no-files-found: error/);
  assert.equal(workflow.match(/name: npm-release\.tar\.gz/g)?.length, 2);
  const stageSection = workflow.slice(workflow.indexOf('  stage:'));
  const submitSection = workflow.slice(workflow.indexOf('  submit:'));
  assert.match(stageSection, /name: npm-release\.tar\.gz\n\s+path: npm-release\.tar\.gz\n\s+archive: false/);
  assert.match(submitSection, /name: npm-release\.tar\.gz\n\s+path: \./);
  assert.match(workflow, /RUST_TOOLCHAIN: "1\.96\.1"/);
  assert.match(workflow, /NODE_VERSION: "24\.8\.0"/);
  assert.match(workflow, /NPM_VERSION: "11\.18\.0"/);
  assert.match(workflow, /npm install --global "npm@\$NPM_VERSION" --ignore-scripts/);
  assert.match(workflow, /rustup component add --toolchain "\$RUST_TOOLCHAIN" clippy/);
  assert.equal(workflow.includes('rustup toolchain install stable'), false);
  assert.match(workflow, /\(cd release && sha256sum \*\.tgz > SHA256SUMS\)/);
  assert.match(workflow, /\(cd release && sha256sum --check SHA256SUMS\)/);
  assert.match(workflow, /name: npm-release\.tar\.gz(?:.|\n)*?retention-days: 30/);
});

test('staged publishing is manual, OIDC-scoped, and orders platform packages before meta', () => {
  const workflow = fs.readFileSync(workflowPath, 'utf8');
  const publishJob = workflow.indexOf('\n  submit:');
  const oidcPermission = workflow.indexOf('id-token: write', publishJob);
  const platformLoop = workflow.indexOf('for package in "${platform_packages[@]}"', publishJob);
  const metaPublish = workflow.indexOf('stage_package "@orchester/cli"', publishJob);

  assert.ok(publishJob > 0);
  assert.match(workflow, /submit:\n\s+description: [^\n]+\n\s+required: true\n\s+default: none\n\s+type: choice\n\s+options:\n\s+- none\n\s+- platforms\n\s+- meta/);
  assert.ok(oidcPermission > publishJob);
  assert.match(
    workflow.slice(publishJob),
    /if: inputs\.submit != 'none' && github\.ref_type == 'tag'/,
  );
  assert.match(workflow.slice(publishJob), /\[\[ "\$TAG" == "v\$VERSION" \]\]/);
  assert.match(workflow, /if \[\[ "\$SUBMIT" != "none" \]\]; then/);
  assert.match(workflow, /\[\[ "\$REF_TYPE" == "tag" \]\]/);
  assert.ok(platformLoop > oidcPermission);
  assert.ok(metaPublish > platformLoop);
  assert.match(workflow.slice(publishJob), /if \[\[ "\$SUBMIT" == "platforms" \]\]; then/);
  const platformPreflight = workflow.indexOf('npm view "$package@$VERSION" version', platformLoop);
  assert.ok(platformPreflight > platformLoop);
  assert.ok(metaPublish > platformPreflight);
  assert.match(workflow.slice(publishJob), /npm stage publish/);
  assert.match(workflow.slice(publishJob), /npm stage publish "\.\/release\/\$archive"/);
  assert.equal(/\bnpm publish\b/.test(workflow.slice(publishJob)), false);
  assert.match(workflow, /concurrency:\n  group: npm-release\n  cancel-in-progress: false/);
  assert.equal(workflow.includes('NODE_AUTH_TOKEN'), false);
  assert.equal(workflow.includes('NPM_TOKEN'), false);
});
