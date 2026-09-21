import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const workflowPath = path.join(repositoryRoot, '.github/workflows/desktop-release.yml');
const configPath = path.join(repositoryRoot, 'apps/desktop/src-tauri/tauri.conf.json');

function readWorkflow() {
  return fs.readFileSync(workflowPath, 'utf8').replace(/\r\n/g, '\n');
}

function readConfig() {
  return JSON.parse(fs.readFileSync(configPath, 'utf8'));
}

function job(workflow, name, next) {
  const start = workflow.indexOf(`\n  ${name}:`);
  assert.notEqual(start, -1, `missing ${name} job`);
  const end = next === undefined ? workflow.length : workflow.indexOf(`\n  ${next}:`, start);
  assert.notEqual(end, -1, `missing ${next} job`);
  return workflow.slice(start, end);
}

test('desktop release config enables per-user NSIS installers with the bundled web payload', () => {
  const bundle = readConfig().bundle;
  assert.equal(bundle.active, true);
  assert.deepEqual(bundle.targets, ['nsis']);
  assert.deepEqual(bundle.resources, { '../../web/dist/': 'web/' });
  assert.equal(bundle.windows.nsis.installMode, 'currentUser');
  assert.equal(bundle.windows.nsis.displayLanguageSelector, false);
  assert.deepEqual(bundle.windows.nsis.languages, ['English', 'SimpChinese']);
  assert.equal(bundle.windows.webviewInstallMode.type, 'downloadBootstrapper');
});

test('desktop release workflow builds both Windows architectures before staging', () => {
  const workflow = readWorkflow();
  assert.match(workflow, /arch: x64\n\s+runner: windows-2025\n\s+rust_target: x86_64-pc-windows-msvc/);
  assert.match(workflow, /arch: arm64\n\s+runner: windows-11-arm\n\s+rust_target: aarch64-pc-windows-msvc/);
  assert.match(workflow, /node werkzeug\/desktop\/build-installer\.mjs/);
  assert.match(workflow, /werkzeug\/desktop\/verify-install\.ps1/);
  assert.match(workflow, /node werkzeug\/desktop\/stage-release\.mjs/);
  assert.match(workflow, /node werkzeug\/desktop\/verify-release\.mjs/);
  assert.match(job(workflow, 'build', 'stage'), /needs: validate\n/);
  assert.match(job(workflow, 'stage', 'tag'), /needs: \[build, test\]\n/);
  assert.match(job(workflow, 'tag', 'release'), /needs: stage\n/);
  assert.match(job(workflow, 'release', 'verify'), /needs: \[stage, tag\]\n/);
  assert.match(job(workflow, 'verify'), /needs: release\n/);
  assert.match(workflow, /TAG: desktop-v\$\{\{ inputs\.version \}\}/);
  for (const line of workflow.split('\n')) {
    if (line.includes('v${{ inputs.version }}')) {
      assert.match(line, /desktop-v\$\{\{ inputs\.version \}\}/, line);
    }
  }
  assert.ok((workflow.match(/ref: \$\{\{ github\.sha \}\}/g)?.length ?? 0) >= 5);
});

test('desktop release workflow publishes immutable assets with limited permissions', () => {
  const workflow = readWorkflow();
  assert.match(workflow, /^permissions:\n  contents: read\n/m);

  const stage = job(workflow, 'stage', 'tag');
  assert.equal(/contents: write/.test(stage), false);
  assert.match(stage, /sha256sum|stage-release\.mjs/);

  const release = job(workflow, 'release', 'verify');
  assert.match(release, /permissions:\n      contents: write/);
  assert.match(release, /gh release create "\$TAG"/);
  assert.match(release, /--verify-tag/);
  assert.match(release, /sha256sum --check SHA256SUMS/);
  assert.match(release, /## Full Changelog/);
  assert.match(release, /git log --pretty=format:'- %s \(%h\)'/);

  const verify = job(workflow, 'verify');
  assert.match(verify, /isDraft/);
  assert.match(verify, /_x64-setup\.exe/);
  assert.match(verify, /_arm64-setup\.exe/);
  assert.match(verify, /verify-release\.mjs/);
});
