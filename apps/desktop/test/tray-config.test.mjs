import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { dirname, resolve } from 'node:path'

const testDirectory = dirname(fileURLToPath(import.meta.url))
const tauriDirectory = resolve(testDirectory, '..', 'src-tauri')

async function readText(relativePath) {
  return readFile(resolve(tauriDirectory, relativePath), 'utf8')
}

async function readJson(relativePath) {
  return JSON.parse(await readText(relativePath))
}

/**
 * Closing the window must not end the run. The window hides instead, and the
 * only exits are the tray menu's own entries — so these tests pin the three
 * pieces that have to agree for that to hold: the tray feature is compiled in,
 * the close handler prevents the close, and the menu carries both the show and
 * the quit entries.
 */
test('the desktop shell builds the tray feature in', async () => {
  const manifest = await readText('Cargo.toml')
  assert.match(manifest, /tauri\s*=\s*\{[^}]*"tray-icon"/s)
})

test('the close request hides the window instead of ending the process', async () => {
  const lib = await readText('src/lib.rs')
  assert.match(lib, /on_window_event/)
  assert.match(lib, /CloseRequested/)
  assert.match(lib, /prevent_close/)
  assert.match(lib, /\.hide\(\)/)
})

test('the tray menu carries a show entry, a settings entry and the only quit', async () => {
  const lib = await readText('src/lib.rs')
  assert.match(lib, /MenuItem::with_id\([^)]*"show"/s)
  assert.match(lib, /MenuItem::with_id\([^)]*"settings"/s)
  assert.match(lib, /MenuItem::with_id\([^)]*"quit"/s)
  assert.match(lib, /app\.exit\(0\)/)
})

test('the tray opens the window on a left click and does not steal the menu', async () => {
  const lib = await readText('src/lib.rs')
  assert.match(lib, /show_menu_on_left_click\(false\)/)
  assert.match(lib, /TrayIconEvent::Click/)
  assert.match(lib, /MouseButton::Left/)
  assert.match(lib, /show_main_window/)
})

test('the tray icon is required rather than optional, so a missing icon fails loudly', async () => {
  const lib = await readText('src/lib.rs')
  assert.match(lib, /default_window_icon/)
  assert.match(lib, /TrayIconBuilder/)
  assert.doesNotMatch(lib, /if let Some\(icon\)/)
})

test('the tray does not add permissions the webview could reach for', async () => {
  const capability = await readJson('capabilities/default.json')
  assert.ok(capability.permissions.every((permission) => permission.startsWith('core:window:')))
  assert.ok(!capability.permissions.some((permission) => permission.includes('tray')))
  assert.ok(!capability.permissions.some((permission) => permission.startsWith('shell:')))
})
