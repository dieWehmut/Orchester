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
  assert.match(workflow, /\n  publish:\n(?:.|\n)*?    needs: stage\n/);
  assert.match(workflow, /\n  tag:\n(?:.|\n)*?    needs: \[stage, publish\]\n/);
  assert.match(workflow, /\n  release:\n(?:.|\n)*?    needs: \[stage, tag\]\n/);
  assert.match(workflow, /\n  verify:\n(?:.|\n)*?    needs: \[publish, release\]\n/);
  assert.ok((workflow.match(/ref: \$\{\{ github\.sha \}\}/g)?.length ?? 0) >= 4);
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
  assert.ok((workflow.match(/name: npm-release\.tar\.gz/g)?.length ?? 0) >= 3);
  const stageSection = workflow.slice(workflow.indexOf('  stage:'));
  const publishSection = workflow.slice(workflow.indexOf('  publish:'));
  const releaseSection = workflow.slice(workflow.indexOf('  release:'));
  assert.match(stageSection, /name: npm-release\.tar\.gz\n\s+path: npm-release\.tar\.gz\n\s+archive: false/);
  assert.match(publishSection, /name: npm-release\.tar\.gz\n\s+path: \./);
  assert.match(releaseSection, /name: npm-release\.tar\.gz\n\s+path: \./);
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

test('release artifact contains the exact verified official plugin matrix', () => {
  const workflow = fs.readFileSync(workflowPath, 'utf8');
  const stageJob = workflow.slice(workflow.indexOf('\n  stage:'), workflow.indexOf('\n  tag:'));
  const verifyPlugins = stageJob.indexOf('node werkzeug/npm/plugin-release.mjs');
  const findPlugins = stageJob.indexOf('find npm/plugins -mindepth 1 -maxdepth 1 -type d -print | sort');
  const packPlugins = stageJob.indexOf('for directory in "${plugin_dirs[@]}"; do');

  assert.ok(verifyPlugins > 0);
  assert.ok(findPlugins > verifyPlugins);
  assert.ok(packPlugins > findPlugins);
  assert.match(stageJob, /\[\[ \$\{#plugin_dirs\[@\]\} -eq 3 \]\]/);
  assert.match(stageJob, /npm pack --ignore-scripts --pack-destination release \.\/npm\/cli/);
  assert.match(stageJob, /npm pack --ignore-scripts --pack-destination release "\.\/\$directory"/);
  assert.match(stageJob, /-name '\*\.tgz' \| wc -l\) -eq 10/);
});

test('one dispatch tags, publishes, releases, and verifies v0.1.0 in dependency order', () => {
  const workflow = fs.readFileSync(workflowPath, 'utf8');
  const tagJob = workflow.indexOf('\n  tag:');
  const publishJob = workflow.indexOf('\n  publish:');
  const publishSection = workflow.slice(publishJob);
  const oidcPermission = workflow.indexOf('id-token: write', publishJob);
  const versionProbe = workflow.indexOf('published_version()', publishJob);
  const pluginCollection = workflow.indexOf('OFFICIAL_AGENT_PLUGINS', publishJob);
  const platformPublish = workflow.indexOf('for package in "${platform_packages[@]}"; do', publishJob);
  const pluginPublish = workflow.indexOf('for package in "${plugin_packages[@]}"; do', publishJob);
  const metaPublish = workflow.indexOf('publish_package "@orchester/cli"', publishJob);

  assert.ok(tagJob > 0);
  assert.ok(publishJob > 0);
  assert.match(workflow, /workflow_dispatch:\n\s+inputs:\n\s+version:\n\s+description: [^\n]+\n\s+required: true\n\s+default: "0\.1\.0"\n\s+type: string/);
  assert.equal(workflow.includes('submit:'), false);
  assert.equal(workflow.includes('inputs.submit'), false);
  assert.match(workflow.slice(tagJob, publishJob), /permissions:\n\s+contents: write/);
  assert.match(workflow.slice(tagJob, publishJob), /git tag -a "\$TAG" "\$GITHUB_SHA"/);
  assert.match(workflow.slice(tagJob, publishJob), /git push origin "refs\/tags\/\$TAG"/);
  assert.ok(oidcPermission > publishJob);
  assert.ok(versionProbe > oidcPermission);
  assert.ok(pluginCollection > oidcPermission);
  assert.match(publishSection, /NODE_AUTH_TOKEN: \$\{\{ secrets\.NPM_TOKEN \}\}/);
  assert.match(publishSection, /environment: npm-release/);
  assert.match(publishSection, /\[\[ \$\{#plugin_packages\[@\]\} -eq 3 \]\]/);
  assert.match(publishSection, /if \[\[ -z "\$\{NODE_AUTH_TOKEN:-\}" \]\]; then/);
  assert.match(publishSection, /NPM_TOKEN is required to bootstrap/);
  assert.ok(platformPublish > versionProbe);
  assert.ok(pluginPublish > platformPublish);
  assert.ok(metaPublish > pluginPublish);
  assert.match(publishSection, /npm view "\$package@\$VERSION" version/);
  assert.match(publishSection, /npm publish "\.\/release\/\$archive" --access public --provenance --ignore-scripts/);
  assert.equal(workflow.includes('npm stage publish'), false);
  assert.match(workflow, /gh release create "\$TAG"/);
  assert.match(workflow, /gh release upload "\$TAG" release\/\* npm-release\.tar\.gz --clobber/);
  assert.match(workflow, /concurrency:\n  group: npm-release-\$\{\{ inputs\.version \}\}\n  cancel-in-progress: false/);
  assert.match(workflow, /npm view "@orchester\/cli@\$VERSION" version/);
});
