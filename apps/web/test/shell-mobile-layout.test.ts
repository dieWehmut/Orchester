// @vitest-environment node
import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

const source = readFileSync(new URL('../src/components/layout/AppShell.vue', import.meta.url), 'utf8')
const mobile = source.slice(source.indexOf('@media (max-width: 799px)'))

describe('shell responsive CSS contract', () => {
  it('places the transcript in the only mobile track when drawer panes leave the grid', () => {
    // Override the three-class desktop folded+inspector-closed rule as well.
    expect(mobile).toMatch(/\.app-shell\.app-shell--rail-closed \.app-shell__grid\s*\{/)
    expect(mobile).toMatch(/grid-template-columns:\s*minmax\(0, 1fr\);/)
    expect(mobile).toMatch(/\.app-shell__transcript\s*\{\s*grid-column:\s*1;/)
    expect(mobile).toMatch(/\.app-shell__rail,\s*\.app-shell__inspector\s*\{\s*display:\s*none;/)
  })

  it('removes the folded rail border so a zero-width track has no residual seam', () => {
    expect(source).toMatch(/\.app-shell--rail-closed \.app-shell__rail\s*\{[^}]*border-inline-end:\s*0;/)
  })

  it('uses the shared slow duration for track motion and delayed rail hiding', () => {
    expect(source).toContain('grid-template-columns var(--transition-slow, 340ms) var(--ease-out)')
    expect(source).toContain('visibility 0s var(--transition-slow, 340ms)')
  })
})
