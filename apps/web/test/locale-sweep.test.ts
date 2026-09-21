import { readFileSync, readdirSync } from 'node:fs'
import { resolve } from 'node:path'

import ShortcutEditor from '../src/components/settings/ShortcutEditor.vue'
import { createI18n } from '../src/i18n'
import { createShortcutRegistry } from '../src/shortcuts/registry'

import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'

/**
 * The locale sweep, wave U8 of the implementation plan.
 *
 * A reader who switches the interface language expects the whole interface to
 * switch with it. The way that stops being true is never a decision: it is one
 * component added later whose label was easier to type than to look up, and by
 * the time anyone notices, the missing key is somewhere in a hundred files.
 *
 * So the rule is checkable from the source, and it is checked here rather than
 * swept once: a user-visible string in the WebUI is a locale key, not a
 * sentence typed into a template.
 *
 * Two things are deliberately not violations. A component's own default prop is
 * a library default, and the surface that renders it passes a translated value
 * over the top - `packages/design` cannot import the WebUI's locale service
 * without inverting the dependency, so the defaults stay as English fallbacks.
 * Data that arrives from the runtime - a session title, an agent's own name, a
 * tool's output - is the runtime's to name, not the interface's to translate.
 */

/** The WebUI source root: this test runs with the package as its cwd. */
const SOURCE_ROOT = resolve(process.cwd(), 'src')

function components(): string[] {
  const found: string[] = []
  const walk = (dir: string): void => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (entry.name === 'locales' || entry.name === 'node_modules') continue
        walk(resolve(dir, entry.name))
      } else if (/\.(vue|ts)$/.test(entry.name)) {
        found.push(resolve(dir, entry.name))
      }
    }
  }
  walk(SOURCE_ROOT)
  return found
}

function relative(path: string): string {
  return path.slice(SOURCE_ROOT.length + 1).split('\\').join('/')
}

/**
 * A static attribute whose value is a sentence rather than a binding or a key.
 *
 * `aria-label="Diff preview"` is a string a reader hears; `:aria-label="label"`
 * and `:aria-label="t('...')"` both resolve at render time and are fine.
 */
const STATIC_ATTRIBUTE =
  /(?<![:\w-])(?:aria-label|aria-description|placeholder|title|alt)="([^"]*)"/g

/** A binding that resolves to a plain quoted sentence. */
const LITERAL_BINDING = /(?<![:\w-])(?:aria-label|title|placeholder)="'[^']*[A-Za-z]{3}[^']*'"/g

/**
 * A prop default in a component the WebUI owns: copy a reader can see.
 *
 * The design package keeps English fallbacks because it cannot import this
 * app's locale service. These components can, so a default that is only some
 * English words is a default the localiser never reaches.
 */
const DEFAULT_PROP = /^\s{2,}(\w+)\??:\s*'([^']*)'\s*,?\s*$/gm

/**
 * A sentence typed inside a template interpolation, which renders as text.
 *
 * A ternary between two quoted sentences is copy in a template even though
 * no attribute carries it.
 */
const INTERPOLATION = /\{\{[\s\S]*?\}\}/g

function englishWords(value: string): boolean {
  return /[A-Za-z]{3,}\s+[A-Za-z]{2,}/.test(value)
}

/** The prop defaults a component declares with English words in them. */
function englishDefaults(source: string): string[] {
  const found: string[] = []
  const block = source.match(/withDefaults\(\s*defineProps<[\s\S]*?>\(\)\s*,\s*\{([\s\S]*?)\n\s*\}\s*\)/)
  for (const match of (block?.[1] ?? '').matchAll(DEFAULT_PROP)) {
    if (englishWords(match[2] ?? '')) found.push(match[0].trim())
  }
  return found
}

/** The English sentences a template interpolates directly. */
function englishInterpolations(source: string): string[] {
  const found: string[] = []
  const template = source.match(/<template>([\s\S]*)<\/template>/)
  for (const match of (template?.[1] ?? '').matchAll(INTERPOLATION)) {
    for (const literal of (match[0] ?? '').matchAll(/'([^']*)'/g)) {
      if (englishWords(literal[1] ?? '')) found.push(match[0].trim())
    }
  }
  return found
}

/** The text between tags in a template, with interpolations removed. */
function templateText(source: string): string[] {
  const match = source.match(/<template>([\s\S]*)<\/template>/)
  if (!match) return []
  const body = match[1] ?? ''
  const withoutInterpolation = body.replace(/\{\{[\s\S]*?\}\}/g, '\u0000')
  const found: string[] = []
  for (const chunk of withoutInterpolation.split(/<[^>]*>/)) {
    const text = chunk.replace(/\s+/g, ' ').trim()
    if (text.length === 0 || text.includes('\u0000')) continue
    if (/^[A-Za-z][A-Za-z0-9 ,.'!?&%:;()\/-]{1,}$/.test(text)) found.push(text)
  }
  return found
}

/**
 * Files that are allowed to carry English words, with the reason.
 *
 * Only the catalogue itself is exempt. Everything else a reader can read has
 * to arrive through a key, including the cross-client vocabulary: the shortcut
 * registry and the composer's command list keep stable identities and keys,
 * and the surface that draws them resolves the words.
 */
const ALLOWED: Readonly<Record<string, string>> = {
  'locales/en.json': 'the English catalogue itself',
}

describe('WebUI locale sweep', () => {
  it('keeps static user-visible attributes behind a locale key', () => {
    const failures: string[] = []
    for (const path of components()) {
      const name = relative(path)
      if (ALLOWED[name] !== undefined) continue
      const source = readFileSync(path, 'utf8')
      for (const match of source.matchAll(STATIC_ATTRIBUTE)) {
        if (/[A-Za-z]{3}/.test(match[1] ?? '')) failures.push(`${name}: ${match[0]}`)
      }
      for (const match of source.matchAll(LITERAL_BINDING)) {
        if (/[A-Za-z]{3}/.test(match[0])) failures.push(`${name}: ${match[0]}`)
      }
    }
    expect(failures).toEqual([])
  })

  it('keeps template text behind a locale key', () => {
    const failures: string[] = []
    for (const path of components()) {
      const name = relative(path)
      if (ALLOWED[name] !== undefined) continue
      for (const text of templateText(readFileSync(path, 'utf8'))) {
        failures.push(`${name}: "${text}"`)
      }
    }
    expect(failures).toEqual([])
  })

  it('keeps the English words out of the prop defaults the app owns', () => {
    const failures: string[] = []
    for (const path of components()) {
      const name = relative(path)
      if (ALLOWED[name] !== undefined) continue
      for (const found of englishDefaults(readFileSync(path, 'utf8'))) {
        failures.push(name + ': ' + found)
      }
    }
    expect(failures).toEqual([])
  })

  it('keeps the English words out of the templated sentences', () => {
    const failures: string[] = []
    for (const path of components()) {
      const name = relative(path)
      if (ALLOWED[name] !== undefined) continue
      for (const found of englishInterpolations(readFileSync(path, 'utf8'))) {
        failures.push(name + ': ' + found)
      }
    }
    expect(failures).toEqual([])
  })

  it('renders the copy in the language the reader chose', () => {
    // The sweep above proves the English words have moved. This proves they
    // moved somewhere that renders: a key that resolves is the difference
    // between a localised interface and an interface with a lookup table.
    const registry = createShortcutRegistry()
    registry.register({
      id: 'palette.open',
      labelKey: 'shortcuts.labels.paletteOpen',
      groupKey: 'shortcuts.groups.composer',
      keys: ['Mod', 'K'],
    })

    const chinese = mount(ShortcutEditor, {
      props: { registry, platform: 'windows' },
      global: { plugins: [createI18n('zh-CN')] },
    })
    expect(chinese.text()).toContain('打开命令面板')
    expect(chinese.text()).not.toContain('Open the command palette')

    const english = mount(ShortcutEditor, {
      props: { registry, platform: 'windows' },
      global: { plugins: [createI18n('en')] },
    })
    expect(english.text()).toContain('Open the command palette')
  })

  it('keeps every locale catalogue on the same key set as English', () => {
    const flatten = (tree: Record<string, unknown>, prefix = ''): string[] =>
      Object.entries(tree).flatMap(([key, value]) =>
        typeof value === 'string'
          ? [`${prefix}${key}`]
          : flatten(value as Record<string, unknown>, `${prefix}${key}.`),
      )
    const read = (locale: string): string[] =>
      flatten(JSON.parse(readFileSync(resolve(SOURCE_ROOT, 'locales', locale), 'utf8'))).sort()

    const english = read('en.json')
    expect(english.length).toBeGreaterThan(0)
    for (const locale of ['zh-CN.json', 'zh-TW.json']) {
      expect({ locale, keys: read(locale) }).toEqual({ locale, keys: english })
    }
  })
})
