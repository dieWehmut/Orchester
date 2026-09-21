import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const css = readFileSync(resolve(process.cwd(), 'src/index.css'), 'utf8')

describe('design reset stylesheet', () => {
  it('contains only global reset and interaction contracts', () => {
    expect(css).toContain(':where(*')
    expect(css).toContain(':where(body)')
    expect(css).toContain('box-sizing: border-box')
    expect(css).toContain('margin: 0')
    expect(css).toContain('prefers-reduced-motion')
  })

  it('does not claim ownership of consumer layout selectors', () => {
    for (const selector of [
      '.app-',
      '#app',
      ':where(main)',
      ':where(section)',
      ':where(header)',
      ':where(nav)',
      ':where(aside)',
    ]) {
      expect(css).not.toContain(selector)
    }
  })
})
