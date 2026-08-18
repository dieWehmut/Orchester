import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'

const tokens = readFileSync(resolve(process.cwd(), 'src/tokens.css'), 'utf8')

describe('operational layout tokens', () => {
  it('defines stable pane, control, breakpoint, and layer tokens', () => {
    for (const token of [
      '--sidebar-min-width',
      '--sidebar-max-width',
      '--inspector-min-width',
      '--inspector-max-width',
      '--control-height-sm',
      '--control-height-md',
      '--control-height-lg',
      '--breakpoint-mobile',
      '--breakpoint-tablet',
      '--breakpoint-desktop',
      '--z-header',
      '--z-drawer',
      '--z-dialog',
      '--z-toast',
    ]) {
      expect(tokens).toContain(token)
    }
  })

  it('keeps operational surfaces at or below the eight pixel radius', () => {
    expect(tokens).toMatch(/--radius-md:\s*8px/)
    expect(tokens).toMatch(/--radius-lg:\s*8px/)
    expect(tokens).toMatch(/--radius-xl:\s*8px/)
  })
})
