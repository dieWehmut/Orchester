import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const css = readFileSync(resolve(process.cwd(), 'src/tokens.css'), 'utf8')

function readBlock(selector: string): Record<string, string> {
  const selectorStart = css.indexOf(selector)
  expect(selectorStart).toBeGreaterThanOrEqual(0)
  const blockStart = css.indexOf('{', selectorStart)
  const blockEnd = css.indexOf('}', blockStart)
  expect(blockStart).toBeGreaterThan(selectorStart)
  expect(blockEnd).toBeGreaterThan(blockStart)

  const block = css.slice(blockStart + 1, blockEnd)
  return Object.fromEntries(
    [...block.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((match) => [
      match[1],
      match[2]?.trim() ?? '',
    ]),
  )
}

const accentKeys = [
  '--color-accent',
  '--color-accent-hover',
  '--color-accent-muted',
  '--color-accent-border',
  '--color-accent-contrast',
  '--color-glow',
]

describe('theme token contracts', () => {
  it('keeps neutral surfaces complete in both themes', () => {
    expect(readBlock("[data-theme='dark']")).toMatchSnapshot()
    expect(readBlock("[data-theme='light']")).toMatchSnapshot()
  })

  it('keeps every accent scheme complete in dark and light modes', () => {
    for (const theme of ['dark', 'light'] as const) {
      for (const scheme of ['amber', 'violet', 'teal', 'rose'] as const) {
        const block = readBlock(
          "[data-theme='" + theme + "'][data-color-scheme='" + scheme + "']",
        )
        expect(Object.fromEntries(accentKeys.map((key) => [key, block[key]]))).toMatchSnapshot(
          theme + '-' + scheme,
        )
      }
    }
  })
})
