import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

function source(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), 'src', relativePath), 'utf8')
}

describe('workspace visual policy', () => {
  it('does not use decorative gradients or viewport-scaled type in the empty workspace', () => {
    const mark = source('components/run/OrchesterMark.vue')
    const empty = source('components/run/EmptyWorkspace.vue')

    expect(mark).not.toMatch(/(?:linear|radial)-gradient\(/)
    expect(empty).not.toMatch(/\b(?:vw|vh|vmin|vmax)\b/)
  })

  it('routes full-height layouts through the shared application chrome offset', () => {
    const app = source('styles/app.css')
    const fullHeightSources = [
      source('components/layout/WorkspaceShell.vue'),
      source('components/layout/WorkspaceResponsive.vue'),
      source('views/SettingsView.vue'),
      source('views/NotFoundView.vue'),
    ]

    expect(app).toContain('--app-top-chrome-height')
    expect(app).toContain('--desktop-titlebar-height: 36px')
    for (const contents of fullHeightSources) {
      expect(contents).toContain('var(--app-top-chrome-height')
      expect(contents).not.toContain('calc(100vh - var(--header-height))')
    }
  })
})
