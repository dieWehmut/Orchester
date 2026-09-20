import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

test('native windows stay opaque when compositor effects are unavailable', async () => {
  for (const file of ['tauri.conf.json', 'tauri.macos.conf.json']) {
    const config = JSON.parse(await readFile(new URL(`../src-tauri/${file}`, import.meta.url), 'utf8'))
    const window = config.app.windows[0]
    assert.equal(window.transparent, false)
    assert.equal(window.backgroundColor, '#101010')
    assert.equal(window.windowEffects, undefined)
  }
})

test('macOS overlay preserves native traffic lights within the 38px titlebar', async () => {
  const base = JSON.parse(await readFile(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'))
  let platform = {}
  try {
    platform = JSON.parse(await readFile(new URL('../src-tauri/tauri.macos.conf.json', import.meta.url), 'utf8'))
  } catch (error) {
    if (error.code !== 'ENOENT') throw error
  }
  const window = (platform.app?.windows ?? base.app.windows)[0]
  assert.equal(window.label, 'main')
  assert.equal(window.titleBarStyle, 'Overlay')
  assert.equal(window.decorations, true)
  assert.equal(window.hiddenTitle, true)
  assert.deepEqual(window.trafficLightPosition, { x: 12, y: 12 })
  assert.equal(window.devtools, false)
})
