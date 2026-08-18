import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const readme = readFileSync(resolve(process.cwd(), 'README.md'), 'utf8')

describe('design package documentation', () => {
  it('documents the public imports and CSS entry points', () => {
    expect(readme).toContain("from '@orchester/design'")
    expect(readme).toContain("@orchester/design/tokens.css")
    expect(readme).toContain("@orchester/design/index.css")
    expect(readme).toContain('@lucide/vue')
  })

  it('documents keyboard contracts and the package boundary', () => {
    for (const contract of [
      'ArrowLeft',
      'ArrowRight',
      'Home',
      'End',
      'Escape',
      'focus trap',
      'does not own network',
    ]) {
      expect(readme).toContain(contract)
    }
  })
})
