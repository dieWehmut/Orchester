import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const moduleDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(moduleDirectory, '../..');
const desktopRoot = path.join(repositoryRoot, 'apps/desktop');

function fail(message) {
  process.stderr.write(`verify-bundle: ${message}\n`);
  process.exit(1);
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, 'utf8'));
}

export function verifyBundleContract() {
  const config = readJson(path.join(desktopRoot, 'src-tauri/tauri.conf.json'));
  const bundle = config.bundle;
  if (bundle?.active !== true) fail('bundle.active must be true to produce an installer');
  if (JSON.stringify(bundle.targets) !== JSON.stringify(['nsis'])) fail('bundle.targets must be exactly ["nsis"]');
  if (bundle.resources?.['../../web/dist/'] !== 'web/') fail('the bundled WebUI payload must map ../../web/dist/ to web/');
  if (bundle.icon?.length !== 2) fail('the desktop icon set must keep the png and ico entries');
  const nsis = bundle.windows?.nsis;
  if (nsis?.installMode !== 'currentUser') fail('NSIS must install per user so no elevation is required');
  if (nsis?.displayLanguageSelector !== false) fail('the installer must not block on a language selector');
  const languages = nsis?.languages;
  if (JSON.stringify(languages) !== JSON.stringify(['English', 'SimpChinese'])) fail('expected the pinned installer languages');
  const webview = bundle.windows?.webviewInstallMode;
  if (webview?.type !== 'downloadBootstrapper' || webview?.silent !== true) {
    fail('the installer must provision WebView2 silently');
  }

  const runtime = fs.readFileSync(path.join(desktopRoot, 'src-tauri/src/runtime.rs'), 'utf8');
  if (!runtime.includes('resource_dir()?.join("web")') && !runtime.includes('resource_directory.join("web")')) {
    fail('the runtime must resolve the bundled payload from the web resource directory');
  }
  const manifest = readJson(path.join(desktopRoot, 'package.json'));
  if (manifest.scripts?.build !== 'tauri build') fail('the desktop build script must remain `tauri build`');

  if (!fs.existsSync(path.join(repositoryRoot, 'apps/web/dist/index.html'))) {
    fail('apps/web/dist/index.html is missing; build the WebUI payload first');
  }
}

const invokedDirectly = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  verifyBundleContract();
  process.stdout.write('verify-bundle: NSIS bundle contract holds and the web payload exists\n');
}
